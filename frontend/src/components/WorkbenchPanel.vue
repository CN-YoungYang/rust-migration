<template>
  <section class="workbench-panel" aria-labelledby="workbench-title">
    <header class="workbench-header">
      <div class="heading-block">
        <p class="eyebrow">今日运营</p>
        <h2 id="workbench-title" class="workbench-title">签到工作台</h2>
        <p class="workbench-subtitle">集中处理待签到账号，批次会在服务端继续执行。</p>
      </div>
      <div class="header-actions">
        <div class="business-date" aria-label="业务日期和时区">
          <span class="muted">业务日期</span>
          <strong>{{ workbench?.businessDate || '获取中' }}</strong>
          <span class="timezone-label">{{ workbench?.timezone || 'Asia/Shanghai' }}</span>
        </div>
        <n-button size="small" :loading="loading" :disabled="creatingBatch" @click="refreshWorkbench">
          <template #icon><n-icon :component="RefreshOutline" /></template>
          刷新数据
        </n-button>
      </div>
    </header>

    <n-alert v-if="!isOnline" type="warning" :show-icon="true" class="state-alert" role="status" aria-live="polite">
      当前网络连接已断开。已经创建的签到批次仍会在服务端继续执行，网络恢复后可继续查询进度。
    </n-alert>

    <n-alert v-if="pendingCreate" type="warning" :show-icon="true" class="state-alert" role="status" aria-live="polite">
      <div class="pending-create-content">
        <div>
          <strong>上一次批次创建请求尚未确认</strong>
          <p>不要重新生成请求。使用同一个幂等标识重试，服务端只会返回原批次。</p>
        </div>
        <n-space :size="8" wrap>
          <n-button size="small" type="warning" :loading="creatingBatch" @click="retryPendingCreate">
            使用同一标识重试
          </n-button>
          <n-button size="small" secondary :disabled="creatingBatch" @click="refreshWorkbench">
            刷新批次状态
          </n-button>
        </n-space>
      </div>
    </n-alert>

    <n-alert v-if="createError" type="error" :show-icon="true" class="state-alert" role="alert" aria-live="assertive">
      {{ createError }}
    </n-alert>

    <n-alert v-if="loadError" type="error" :show-icon="true" class="state-alert" role="alert" aria-live="assertive">
      <div class="error-content">
        <span>{{ loadError }}</span>
        <n-button size="small" secondary @click="refreshWorkbench">重新加载</n-button>
      </div>
    </n-alert>

    <div v-if="initialLoading" class="loading-shell" aria-label="正在加载签到工作台" aria-busy="true">
      <div class="skeleton-heading"><n-skeleton text :repeat="2" /></div>
      <div class="skeleton-metrics">
        <n-skeleton v-for="item in 4" :key="item" height="92px" :sharp="false" />
      </div>
      <n-skeleton height="68px" :sharp="false" />
      <div class="skeleton-columns">
        <n-skeleton height="360px" :sharp="false" />
        <n-skeleton height="360px" :sharp="false" />
      </div>
    </div>

    <template v-else-if="workbench">
      <div class="context-strip">
        <div class="context-item">
          <span class="muted">最后刷新</span>
          <strong>{{ formatBusinessTime(workbench.generatedAt, true) }}</strong>
        </div>
        <div class="context-item">
          <span class="muted">可见账号</span>
          <strong>{{ workbench.summary.totalAccounts }} 个</strong>
        </div>
        <div class="context-item schedule-context">
          <span class="muted">自动调度</span>
          <n-tag size="small" :bordered="false" :type="workbench.autoSchedule.enabled ? 'success' : 'default'">
            {{ workbench.autoSchedule.enabled ? '已开启' : '未开启' }}
          </n-tag>
          <span v-if="workbench.autoSchedule.enabled" class="muted">
            下次 {{ formatBusinessTime(workbench.autoSchedule.nextRunAt) }}
          </span>
        </div>
      </div>

      <div class="metric-grid" aria-label="今日签到概览">
        <button
          type="button"
          class="metric-card metric-success"
          :class="{ active: filters.todayResult === 'success' }"
          :aria-pressed="filters.todayResult === 'success'"
          @click="applyMetric('success')"
        >
          <span class="metric-label">今日成功</span>
          <strong class="metric-value">{{ workbench.summary.todaySuccess }}</strong>
          <span class="metric-note">本次新成功</span>
        </button>
        <button
          type="button"
          class="metric-card metric-danger"
          :class="{ active: filters.todayResult === 'failed' }"
          :aria-pressed="filters.todayResult === 'failed'"
          @click="applyMetric('failed')"
        >
          <span class="metric-label">今日失败</span>
          <strong class="metric-value">{{ workbench.summary.todayFailed }}</strong>
          <span class="metric-note">优先处理可重试项</span>
        </button>
        <button
          type="button"
          class="metric-card metric-pending"
          :class="{ active: filters.todayResult === 'pending' }"
          :aria-pressed="filters.todayResult === 'pending'"
          @click="applyMetric('pending')"
        >
          <span class="metric-label">待处理</span>
          <strong class="metric-value">{{ workbench.summary.todayPending }}</strong>
          <span class="metric-note">含未签到和等待中账号</span>
        </button>
        <button
          type="button"
          class="metric-card metric-warning"
          :class="{ active: filters.balanceWarning === 'true' }"
          :aria-pressed="filters.balanceWarning === 'true'"
          @click="applyMetric('balanceWarning')"
        >
          <span class="metric-label">余额预警</span>
          <strong class="metric-value">{{ workbench.summary.balanceWarnings }}</strong>
          <span class="metric-note">独立于签到状态</span>
        </button>
      </div>

      <n-card size="small" class="filters-card" :bordered="true">
        <div class="filters-heading">
          <div>
            <strong>筛选账号</strong>
            <span class="muted">筛选只影响当前列表，不改变已经创建的批次范围。</span>
          </div>
          <n-button v-if="hasActiveFilter" size="small" text type="primary" @click="clearFilters">
            清除筛选
          </n-button>
        </div>
        <div class="filters-grid">
          <n-input
            v-model:value="filters.keyword"
            clearable
            size="small"
            placeholder="搜索账号名称、地址或备注"
            aria-label="搜索账号名称、地址或备注"
          >
            <template #prefix><n-icon :component="SearchOutline" /></template>
          </n-input>
          <n-select v-model:value="filters.siteType" size="small" :options="siteTypeOptions" aria-label="站点类型" />
          <n-select
            v-if="isAdmin"
            v-model:value="filters.userId"
            size="small"
            :options="userOptions"
            :loading="usersLoading"
            aria-label="所属用户"
          />
          <n-select v-model:value="filters.enabled" size="small" :options="enabledOptions" aria-label="启用状态" />
          <n-select v-model:value="filters.status" size="small" :options="accountStatusOptions" aria-label="账号状态" />
          <n-select v-model:value="filters.todayResult" size="small" :options="todayResultOptions" aria-label="今日签到结果" />
          <n-select v-model:value="filters.balanceWarning" size="small" :options="balanceOptions" aria-label="余额预警" />
        </div>
      </n-card>

      <div class="dashboard-grid">
        <section class="queue-panel" aria-labelledby="queue-title">
          <div class="section-heading queue-heading">
            <div>
              <h3 id="queue-title">待处理账号</h3>
              <p class="muted">当前页 {{ visibleAccounts.length }} 个，共 {{ workbench.totalAccounts }} 个</p>
            </div>
            <div class="queue-actions">
              <n-button size="small" secondary :disabled="visibleAccounts.length === 0 || creatingBatch" @click="selectAllVisible">
                选择可执行
              </n-button>
              <n-button size="small" quaternary :disabled="selectedIds.length === 0" @click="clearSelection">
                清空选择
              </n-button>
              <n-button
                size="small"
                type="primary"
                :loading="creatingBatch"
                :disabled="selectedIds.length === 0 || actionBusy"
                @click="openPreview(selectedIds, 'checkin')"
              >
                准备签到 {{ selectedIds.length }} 个
              </n-button>
            </div>
          </div>

          <n-alert v-if="workbench.accounts.length > 0 && selectableVisibleCount < visibleAccounts.length" type="info" :show-icon="true" class="queue-hint">
            {{ visibleAccounts.length - selectableVisibleCount }} 个账号因停用、今日已签、重试设置或每日上限暂不可执行。
          </n-alert>

          <div v-if="visibleAccounts.length > 0" class="desktop-queue">
            <n-data-table
              :columns="queueColumns"
              :data="visibleAccounts"
              :loading="loading"
              :row-key="rowKey"
              :checked-row-keys="selectedIds"
              :scroll-x="980"
              class="queue-table"
              @update:checked-row-keys="onCheckedRowKeys"
            />
          </div>

          <div v-if="visibleAccounts.length > 0" class="mobile-queue" aria-label="账号列表">
            <article v-for="account in visibleAccounts" :key="account.id" class="account-card">
              <div class="account-card-top">
                <n-checkbox
                  :checked="isSelected(account.id)"
                  :disabled="!isSelectable(account) || actionBusy"
                  :aria-label="`选择账号 ${account.name}`"
                  @update:checked="(checked) => setSelected(account.id, checked)"
                />
                <div class="account-card-title">
                  <strong>{{ account.name }}</strong>
                  <span class="muted">{{ account.siteType }}</span>
                </div>
                <n-tag size="small" :bordered="false" :type="todayResultTagType(account.todayResult)">
                  {{ todayResultText(account.todayResult) }}
                </n-tag>
              </div>
              <div class="account-card-meta">
                <span class="account-url" :title="account.baseUrl || ''">{{ account.baseUrl || '未设置地址' }}</span>
                <span>今日 {{ account.todayRuns }} 次</span>
                <span :class="{ 'warning-text': account.balanceWarning }">余额 {{ formatBalance(account.lastBalance) }}</span>
              </div>
              <p v-if="account.skipReason" class="skip-reason">{{ account.skipReason }}</p>
              <div class="account-card-actions">
                <n-button size="small" type="primary" secondary :disabled="!isSelectable(account) || actionBusy" @click="openPreview([account.id], 'checkin')">
                  签到
                </n-button>
                <n-button size="small" secondary :loading="busyAccountIds.has(account.id)" :disabled="actionBusy" @click="refreshBalance(account)">
                  刷新余额
                </n-button>
                <n-button size="small" quaternary :disabled="actionBusy" @click="openAccounts(account.id)">
                  编辑
                </n-button>
                <n-button size="small" quaternary :disabled="actionBusy" @click="openRuns(account.id)">
                  查看记录
                </n-button>
              </div>
            </article>
          </div>

          <n-empty v-if="visibleAccounts.length === 0" :description="workbench.totalAccounts > 0 ? '当前筛选没有匹配账号' : '还没有签到账号'" class="queue-empty">
            <template #extra>
              <n-space>
                <n-button v-if="hasActiveFilter" size="small" @click="clearFilters">清除筛选</n-button>
                <n-button v-if="workbench.totalAccounts === 0" size="small" type="primary" @click="openAccounts()">前往账户管理</n-button>
                <n-button v-if="workbench.totalAccounts > 0 && page > 1" size="small" @click="changePage(1)">回到第一页</n-button>
              </n-space>
            </template>
          </n-empty>

          <div v-if="pageCount > 1" class="pagination-row">
            <span class="muted">第 {{ page }} / {{ pageCount }} 页</span>
            <n-pagination :page="page" :page-count="pageCount" :disabled="loading" size="small" @update:page="changePage" />
          </div>

          <div v-if="visibleAccounts.length > 0" class="mobile-action-bar">
            <span>已选 {{ selectedIds.length }} 个</span>
            <n-button size="small" type="primary" :loading="creatingBatch" :disabled="selectedIds.length === 0 || actionBusy" @click="openPreview(selectedIds, 'checkin')">
              准备签到
            </n-button>
          </div>
        </section>

        <aside class="sidebar-stack" aria-label="签到运行信息">
          <n-card size="small" class="schedule-card" :bordered="true">
            <template #header>
              <div class="card-heading"><span>自动调度</span><n-tag size="small" :bordered="false" :type="workbench.autoSchedule.enabled ? 'success' : 'default'">{{ workbench.autoSchedule.enabled ? '已开启' : '未开启' }}</n-tag></div>
            </template>
            <div class="schedule-main">
              <span class="muted">下一次执行</span>
              <strong>{{ workbench.autoSchedule.enabled ? formatBusinessTime(workbench.autoSchedule.nextRunAt, true) : '未安排' }}</strong>
            </div>
            <p class="schedule-cron">计划：{{ workbench.autoSchedule.scheduleCron.join('，') || '未配置' }}</p>
            <div class="schedule-last">
              <span class="muted">最近结果</span>
              <span v-if="workbench.autoSchedule.lastResult">
                <n-tag size="small" :bordered="false" :type="todayResultTagType(workbench.autoSchedule.lastResult.status)">
                  {{ todayResultText(workbench.autoSchedule.lastResult.status) }}
                </n-tag>
                {{ formatBusinessTime(workbench.autoSchedule.lastResult.createdAt) }}
              </span>
              <span v-else class="muted">暂无定时记录</span>
            </div>
            <n-button size="small" text type="primary" class="settings-link" @click="openSettings">在全局设置中编辑调度</n-button>
          </n-card>

          <n-card size="small" class="side-card" :bordered="true">
            <template #header>
              <div class="card-heading"><span>执行中批次</span><span class="muted">{{ activeBatches.length }}</span></div>
            </template>
            <div v-if="activeBatches.length > 0" class="batch-list">
              <button v-for="batch in activeBatches" :key="batch.batchId" type="button" class="batch-row" @click="openBatch(batch.batchId)">
                <div class="batch-row-top">
                  <strong>批次 {{ shortBatchId(batch.batchId) }}</strong>
                  <n-tag size="small" :bordered="false" :type="batchStatusTagType(displayBatch(batch).status)">{{ batchStatusText(displayBatch(batch).status) }}</n-tag>
                </div>
                <div class="batch-row-meta"><span>{{ displayBatch(batch).completed }} / {{ displayBatch(batch).total }} 个完成</span><span>{{ formatBusinessTime(displayBatch(batch).createdAt) }}</span></div>
                <n-progress type="line" :percentage="batchProgress(displayBatch(batch))" :height="6" :show-indicator="false" />
              </button>
            </div>
            <n-empty v-else description="当前没有执行中的批次" size="small" />
          </n-card>

          <n-card size="small" class="side-card" :bordered="true">
            <template #header>
              <div class="card-heading"><span>最近批次</span><n-button size="tiny" text type="primary" @click="refreshWorkbench">刷新</n-button></div>
            </template>
            <div v-if="recentBatches.length > 0" class="batch-list compact-list">
              <button v-for="batch in recentBatches" :key="batch.batchId" type="button" class="batch-row" @click="openBatch(batch.batchId)">
                <div class="batch-row-top">
                  <strong>批次 {{ shortBatchId(batch.batchId) }}</strong>
                  <n-tag size="small" :bordered="false" :type="batchStatusTagType(displayBatch(batch).status)">{{ batchStatusText(displayBatch(batch).status) }}</n-tag>
                </div>
                <div class="batch-row-meta"><span>成功 {{ displayBatch(batch).succeeded }}，失败 {{ displayBatch(batch).failed }}</span><span>{{ formatBusinessTime(displayBatch(batch).createdAt) }}</span></div>
              </button>
            </div>
            <n-empty v-else description="暂无批次记录" size="small" />
          </n-card>

          <n-card size="small" class="side-card warning-card" :bordered="true">
            <template #header>
              <div class="card-heading"><span>余额预警</span><n-tag size="small" :bordered="false" type="warning">{{ workbench.balanceWarnings.length }}</n-tag></div>
            </template>
            <div v-if="workbench.balanceWarnings.length > 0" class="risk-list">
              <div v-for="account in workbench.balanceWarnings" :key="account.id" class="risk-row">
                <div class="risk-main"><strong>{{ account.name }}</strong><span class="muted">{{ formatBalance(account.lastBalance) }}</span></div>
                <n-button size="tiny" text type="warning" @click="focusAccount(account.id)">查看账号</n-button>
              </div>
            </div>
            <n-empty v-else description="暂无余额预警" size="small" />
          </n-card>

          <n-card size="small" class="side-card failure-card" :bordered="true">
            <template #header>
              <div class="card-heading"><span>最近失败</span><span class="muted">{{ workbench.recentFailures.length }}</span></div>
            </template>
            <div v-if="workbench.recentFailures.length > 0" class="failure-list">
              <div v-for="failure in workbench.recentFailures" :key="failure.id" class="failure-row">
                <div class="failure-main">
                  <strong>{{ failure.accountName }}</strong>
                  <span class="muted">{{ failure.siteType }}，{{ formatBusinessTime(failure.createdAt) }}</span>
                  <p :title="failure.message || ''">{{ failure.message || '执行失败，未提供详细原因' }}</p>
                </div>
                <n-space :size="4">
                  <n-button size="tiny" text @click="openRuns(failure.accountId, 'failed')">记录</n-button>
                  <n-button size="tiny" type="primary" text :disabled="!failure.retryEnabled || actionBusy" @click="openPreview([failure.accountId], 'retry')">重试</n-button>
                </n-space>
              </div>
            </div>
            <n-empty v-else description="暂无失败记录" size="small" />
          </n-card>
        </aside>
      </div>
    </template>

    <n-modal v-model:show="previewOpen" preset="card" :title="previewMode === 'retry' ? '确认重试范围' : '确认签到范围'" style="width: 600px; max-width: 94vw" :mask-closable="!creatingBatch">
      <div class="preview-intro">
        <strong>{{ previewMode === 'retry' ? '这次重试会创建一个新的异步批次' : '确认后会创建一个异步签到批次' }}</strong>
        <p>服务端会锁定下面的账号快照。请求只负责入队，不会等待外部站点签到完成。</p>
      </div>
      <div class="preview-summary-grid">
        <div><span class="muted">范围账号</span><strong>{{ previewAccounts.length }}</strong></div>
        <div><span class="muted">预计执行</span><strong class="success-number">{{ previewExecutableCount }}</strong></div>
        <div><span class="muted">预计跳过</span><strong class="warning-number">{{ previewSkippedAccounts.length }}</strong></div>
      </div>
      <n-alert type="info" :show-icon="true" class="preview-note">
        今日已签到、停用、达到每日上限或关闭重试的账号不会被强制执行。最终结果以批次详情中的服务端判断为准。
      </n-alert>
      <div v-if="previewAccounts.length > 0" class="preview-account-list">
        <div v-for="account in previewAccounts" :key="account.id" class="preview-account-row">
          <div>
            <strong>{{ account.name }}</strong>
            <span class="muted">{{ account.siteType || '未知站点' }}</span>
          </div>
          <n-tag size="small" :bordered="false" :type="isSelectable(account) ? 'success' : 'warning'">
            {{ isSelectable(account) ? '可执行' : (account.skipReason || '创建时复核') }}
          </n-tag>
        </div>
      </div>
      <template #footer>
        <n-space justify="end">
          <n-button :disabled="creatingBatch" @click="previewOpen = false">取消</n-button>
          <n-button type="primary" :loading="creatingBatch" @click="confirmPreview">
            {{ creatingBatch ? '创建中…' : (previewMode === 'retry' ? '确认重试' : '确认创建批次') }}
          </n-button>
        </n-space>
      </template>
    </n-modal>

    <n-drawer v-model:show="detailDrawerOpen" :width="560" placement="right">
      <n-drawer-content :title="currentBatchDetail ? `批次 ${shortBatchId(currentBatchDetail.batchId)}` : '批次详情'" closable>
        <div v-if="detailLoading && !currentBatchDetail" class="drawer-loading">
          <n-skeleton text :repeat="3" />
          <n-skeleton height="160px" :sharp="false" />
        </div>
        <n-alert v-if="detailError" type="error" :show-icon="true" class="drawer-alert" role="alert">
          <div class="error-content">
            <span>{{ detailError }}</span>
            <n-button size="small" secondary @click="refreshCurrentBatch">重新查询</n-button>
          </div>
        </n-alert>
        <template v-if="currentBatchDetail">
          <div class="drawer-summary-head">
            <div>
              <n-tag :bordered="false" :type="batchStatusTagType(currentBatchDetail.status)">
                {{ batchStatusText(currentBatchDetail.status) }}
              </n-tag>
              <button type="button" class="batch-id" :title="currentBatchDetail.batchId" @click="copyBatchId">{{ currentBatchDetail.batchId }}</button>
            </div>
            <n-space :size="6" wrap>
              <n-button size="small" secondary :loading="detailLoading" @click="refreshCurrentBatch">刷新</n-button>
              <n-button v-if="currentBatchDetail.canResume && (currentBatchDetail.status === 'pending' || currentBatchDetail.status === 'running')" size="small" type="warning" secondary :loading="resumingBatch" @click="resumeBatch">
                {{ currentBatchDetail.status === 'pending' ? '重新入队' : '恢复执行' }}
              </n-button>
              <n-button size="small" secondary :disabled="retryableDetailCount === 0 || actionBusy" @click="openPreview(retryableDetailIds, 'retry')">
                重试失败 {{ retryableDetailCount }} 个
              </n-button>
              <n-button size="small" tertiary @click="openRunsForBatch">签到记录</n-button>
            </n-space>
          </div>
          <div class="drawer-progress-copy"><span>完成 {{ currentBatchDetail.completed }} / {{ currentBatchDetail.total }}</span><span>{{ formatBusinessTime(currentBatchDetail.createdAt, true) }}</span></div>
          <n-progress type="line" :percentage="batchProgress(currentBatchDetail)" :height="8" :show-indicator="false" />
          <div class="drawer-stats">
            <div><span class="muted">成功</span><strong>{{ currentBatchDetail.succeeded }}</strong></div>
            <div><span class="muted">今日已签</span><strong>{{ currentBatchDetail.alreadyChecked }}</strong></div>
            <div><span class="muted">跳过</span><strong>{{ currentBatchDetail.skipped }}</strong></div>
            <div><span class="muted">失败</span><strong class="danger-number">{{ currentBatchDetail.failed }}</strong></div>
          </div>
          <n-list v-if="currentBatchDetail.items.length > 0" class="detail-list" hoverable>
            <n-list-item v-for="item in currentBatchDetail.items" :key="item.accountId">
              <div class="detail-item">
                <div class="detail-item-main">
                  <div class="detail-item-title"><strong>{{ item.accountName }}</strong><n-tag size="small" :bordered="false" :type="itemTagType(item.status)">{{ itemStatusText(item.status) }}</n-tag></div>
                  <p v-if="item.message" :title="item.message">{{ item.message }}</p>
                  <span v-if="item.runId" class="muted">签到记录已关联</span>
                </div>
                <n-space :size="4">
                  <n-button v-if="item.status === 'failed'" size="tiny" text type="primary" :disabled="!canRetryAccount(item.accountId) || actionBusy" @click="openPreview([item.accountId], 'retry')">重试</n-button>
                  <n-button size="tiny" text :disabled="actionBusy" @click="openRuns(item.accountId, item.status === 'failed' ? 'failed' : '', item.batchId)">记录</n-button>
                </n-space>
              </div>
            </n-list-item>
          </n-list>
          <n-empty v-else description="批次还没有范围明细" />
        </template>
      </n-drawer-content>
    </n-drawer>
  </section>
</template>

<script setup lang="ts">
import { computed, h, onMounted, onUnmounted, reactive, ref, watch } from 'vue'
import {
  NAlert,
  NButton,
  NCard,
  NCheckbox,
  NDataTable,
  NDrawer,
  NDrawerContent,
  NEmpty,
  NIcon,
  NInput,
  NList,
  NListItem,
  NModal,
  NPagination,
  NProgress,
  NSpace,
  NSkeleton,
  NSelect,
  NTag,
  useMessage,
  useThemeVars,
  type DataTableColumns,
} from 'naive-ui'
import { RefreshOutline, SearchOutline } from '@vicons/ionicons5'
import { apiUrl, request, responseData } from '../utils/api'
import { copyText } from '../utils/clipboard'
import { checkinStatusTagType, checkinStatusText } from '../utils/checkinStatus'
import type { Account, CurrentUser } from '../types'
import {
  batchStatusTagType,
  batchStatusText,
  buildWorkbenchQuery,
  isTerminalBatchStatus,
  mergeTrackedBatchIds,
  retryableFailedAccountIds,
  selectableAccountIds,
  type WorkbenchFilters,
} from '../utils/workbench'
import { useUsers } from '../composables/useUsers'

interface WorkbenchAccount extends Account {
  id: string
  name: string
  siteType: string
  enabled: boolean
  retryEnabled: boolean
  balanceWarning: boolean
  todayResult: string
  selectable: boolean
  skipReason?: string | null
  todayRuns: number
}

interface WorkbenchSummary {
  totalAccounts: number
  enabledAccounts: number
  todaySuccess: number
  todayAlreadyChecked: number
  todayFailed: number
  todayPending: number
  balanceWarnings: number
}

interface BatchSummary {
  batchId: string
  createdBy?: string
  triggeredBy?: string
  status: string
  total: number
  completed: number
  succeeded: number
  alreadyChecked: number
  skipped: number
  failed: number
  createdAt: string
  startedAt?: string | null
  finishedAt?: string | null
  canResume?: boolean
}

interface BatchItem {
  batchId: string
  accountId: string
  accountName: string
  position: number
  status: string
  message?: string | null
  runId?: string | null
  createdAt: string
  updatedAt: string
}

interface BatchDetail extends BatchSummary {
  items: BatchItem[]
}

interface WorkbenchFailure {
  id: string
  accountId: string
  accountName: string
  siteType: string
  status: string
  message?: string | null
  durationMs?: number | null
  triggeredBy: string
  createdAt: string
  retryEnabled: boolean
}

interface ScheduledRun {
  id: string
  accountId: string
  accountName: string
  status: string
  message?: string | null
  createdAt: string
}

interface ScheduleSummary {
  enabled: boolean
  timezone: string
  scheduleCron: string[]
  nextRunAt?: string | null
  lastResult?: ScheduledRun | null
}

interface WorkbenchResponse {
  businessDate: string
  timezone: string
  generatedAt: string
  summary: WorkbenchSummary
  accounts: WorkbenchAccount[]
  totalAccounts: number
  limit: number
  offset: number
  activeBatches: BatchSummary[]
  recentBatches: BatchSummary[]
  balanceWarnings: WorkbenchAccount[]
  recentFailures: WorkbenchFailure[]
  autoSchedule: ScheduleSummary
}

interface PendingCreate {
  accountIds: string[]
  idempotencyKey: string
  createdAt: string
}

type PreviewMode = 'checkin' | 'retry'

const props = defineProps<{
  currentUser: CurrentUser | null
  isAdmin: boolean
}>()

const emit = defineEmits<{
  'navigate-runs': [filter: { accountId?: string; status?: string; batchId?: string }]
  'navigate-accounts': [accountId?: string]
  'navigate-settings': []
}>()

const message = useMessage()
const themeVars = useThemeVars()
const { allUsers, usersLoading, loadUsers } = useUsers(() => props.isAdmin)

const PAGE_SIZE = 50
const CONTROL_TIMEOUT_MS = 8000
const BATCH_TIMEOUT_MS = 6000
const POLL_INTERVAL_MS = 2500
const STORAGE_LIMIT = 12
const trackedBatchIds = ref<string[]>([])
const batchDetails = ref<Record<string, BatchDetail>>({})
const workbench = ref<WorkbenchResponse | null>(null)
const loading = ref(false)
const loadError = ref('')
const page = ref(1)
const selectedIds = ref<string[]>([])
const selectionTouched = ref(false)
const creatingBatch = ref(false)
const resumingBatch = ref(false)
const busyAccountIds = ref<Set<string>>(new Set())
const previewOpen = ref(false)
const previewIds = ref<string[]>([])
const previewMode = ref<PreviewMode>('checkin')
const createError = ref('')
const pendingCreate = ref<PendingCreate | null>(null)
const detailDrawerOpen = ref(false)
const activeDetailId = ref('')
const detailLoading = ref(false)
const detailError = ref('')
const polling = ref(false)
const isOnline = ref(navigator.onLine)
let requestSeq = 0
let filterDebounce: ReturnType<typeof setTimeout> | null = null
let pollTimer: ReturnType<typeof setInterval> | null = null
let terminalRefreshQueued = false

const filters = reactive<WorkbenchFilters>({
  keyword: '',
  siteType: '',
  userId: '',
  enabled: '',
  status: '',
  todayResult: '',
  balanceWarning: '',
})

const initialLoading = computed(() => loading.value && !workbench.value)
const actionBusy = computed(() => creatingBatch.value || resumingBatch.value || busyAccountIds.value.size > 0)
const visibleAccounts = computed(() => workbench.value?.accounts || [])
const selectableVisibleCount = computed(() => selectableAccountIds(visibleAccounts.value).length)
const pageCount = computed(() => Math.max(1, Math.ceil((workbench.value?.totalAccounts || 0) / PAGE_SIZE)))
const hasActiveFilter = computed(() => Object.values(filters).some((value) => Boolean(value)))
const knownAccounts = computed(() => {
  const map = new Map<string, WorkbenchAccount>()
  for (const account of visibleAccounts.value) map.set(account.id, account)
  for (const account of workbench.value?.balanceWarnings || []) map.set(account.id, account)
  return [...map.values()]
})

const previewAccounts = computed(() => previewIds.value.map((id) => accountForPreview(id)))
const previewSkippedAccounts = computed(() => previewAccounts.value.filter((account) => !isSelectable(account)))
const previewExecutableCount = computed(() => previewAccounts.value.filter((account) => isSelectable(account)).length)
const currentBatchDetail = computed(() => batchDetails.value[activeDetailId.value] || null)
const retryableDetailIds = computed(() => {
  if (!currentBatchDetail.value) return []
  return retryableFailedAccountIds(currentBatchDetail.value.items, knownAccounts.value)
})
const retryableDetailCount = computed(() => retryableDetailIds.value.length)

const userOptions = computed(() => [
  { label: '全部用户', value: '' },
  ...allUsers.value.map((user) => ({
    label: user.id === props.currentUser?.id ? `${user.username}（我）` : user.username,
    value: user.id,
  })),
])

const siteTypeOptions = [
  { label: '全部站点', value: '' },
  { label: 'new-api', value: 'new-api' },
  { label: 'anyrouter', value: 'anyrouter' },
  { label: 'x666', value: 'x666' },
]

const enabledOptions = [
  { label: '全部启用状态', value: '' },
  { label: '已启用', value: 'true' },
  { label: '已停用', value: 'false' },
]

const accountStatusOptions = [
  { label: '全部签到状态', value: '' },
  { label: '未执行', value: 'never' },
  { label: '成功', value: 'success' },
  { label: '今日已签到', value: 'already_checked' },
  { label: '失败', value: 'failed' },
  { label: '进行中', value: 'pending' },
]

const todayResultOptions = [
  { label: '全部今日结果', value: '' },
  { label: '待签到', value: 'not_checked' },
  { label: '本次成功', value: 'success' },
  { label: '今日已签到', value: 'already_checked' },
  { label: '失败', value: 'failed' },
  { label: '待处理（含进行中）', value: 'pending' },
]

const balanceOptions = [
  { label: '全部余额状态', value: '' },
  { label: '余额预警', value: 'true' },
  { label: '余额正常', value: 'false' },
]

const activeBatches = computed(() => {
  const map = new Map<string, BatchSummary>()
  for (const batch of workbench.value?.activeBatches || []) map.set(batch.batchId, batch)
  for (const id of trackedBatchIds.value) {
    const detail = batchDetails.value[id]
    if (detail && !isTerminalBatchStatus(detail.status)) map.set(id, detail)
  }
  return [...map.values()].sort((a, b) => b.createdAt.localeCompare(a.createdAt))
})

const recentBatches = computed(() => {
  const map = new Map<string, BatchSummary>()
  for (const batch of workbench.value?.recentBatches || []) map.set(batch.batchId, batch)
  for (const id of trackedBatchIds.value) {
    const detail = batchDetails.value[id]
    if (detail && isTerminalBatchStatus(detail.status)) map.set(id, detail)
  }
  return [...map.values()]
    .sort((a, b) => b.createdAt.localeCompare(a.createdAt))
    .slice(0, 8)
})

function storageKey(suffix: string): string {
  return `ai-hub:workbench:${props.currentUser?.id || 'anonymous'}:${suffix}`
}

function restoreLocalState() {
  try {
    const rawIds = localStorage.getItem(storageKey('batches'))
    if (rawIds) {
      const parsed = JSON.parse(rawIds)
      if (Array.isArray(parsed)) trackedBatchIds.value = mergeTrackedBatchIds([], parsed.filter((id): id is string => typeof id === 'string'), STORAGE_LIMIT)
    }
    const rawPending = localStorage.getItem(storageKey('pending-create'))
    if (rawPending) {
      const parsed = JSON.parse(rawPending) as Partial<PendingCreate>
      if (Array.isArray(parsed.accountIds) && typeof parsed.idempotencyKey === 'string' && parsed.idempotencyKey.trim()) {
        pendingCreate.value = {
          accountIds: parsed.accountIds.filter((id): id is string => typeof id === 'string' && Boolean(id.trim())),
          idempotencyKey: parsed.idempotencyKey,
          createdAt: typeof parsed.createdAt === 'string' ? parsed.createdAt : new Date().toISOString(),
        }
      }
    }
  } catch {
    trackedBatchIds.value = []
    pendingCreate.value = null
  }
}

function persistTrackedBatchIds() {
  try {
    localStorage.setItem(storageKey('batches'), JSON.stringify(trackedBatchIds.value))
  } catch {
    // 本地存储不可用时不影响服务端批次执行。
  }
}

function persistPendingCreate() {
  try {
    if (pendingCreate.value) localStorage.setItem(storageKey('pending-create'), JSON.stringify(pendingCreate.value))
    else localStorage.removeItem(storageKey('pending-create'))
  } catch {
    // 本地存储不可用时仍保留当前页面的重试入口。
  }
}

function rememberBatch(id: string) {
  trackedBatchIds.value = mergeTrackedBatchIds(trackedBatchIds.value, [id], STORAGE_LIMIT)
  persistTrackedBatchIds()
}

function ensureTrackedBatches(ids: readonly string[]) {
  const next = mergeTrackedBatchIds(trackedBatchIds.value, ids, STORAGE_LIMIT)
  if (next.join('|') !== trackedBatchIds.value.join('|')) {
    trackedBatchIds.value = next
    persistTrackedBatchIds()
  }
}

function setBatchDetail(detail: BatchDetail) {
  batchDetails.value = { ...batchDetails.value, [detail.batchId]: detail }
}

function requestSignal(timeout: number): AbortSignal {
  return AbortSignal.timeout(timeout)
}

function isTimeoutError(error: unknown): boolean {
  return error instanceof DOMException && (error.name === 'TimeoutError' || error.name === 'AbortError')
}

function formatBusinessTime(value: string | null | undefined, withSeconds = false): string {
  if (!value) return '未安排'
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return '时间无效'
  try {
    return new Intl.DateTimeFormat('zh-CN', {
      timeZone: workbench.value?.timezone || 'Asia/Shanghai',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      ...(withSeconds ? { second: '2-digit' as const } : {}),
    }).format(date)
  } catch {
    return date.toLocaleString('zh-CN')
  }
}

function formatBalance(value: number | string | null | undefined): string {
  if (value === null || value === undefined || value === '') return '未刷新'
  const quota = typeof value === 'string' ? Number(value) : value
  if (!Number.isFinite(quota)) return '未刷新'
  return `$${(quota / 500000).toFixed(2)}`
}

function todayResultText(value: string | null | undefined): string {
  const labels: Record<string, string> = {
    not_checked: '待签到',
    success: '本次成功',
    already_checked: '今日已签',
    failed: '失败',
    pending: '进行中',
    disabled: '已停用',
  }
  return labels[value?.toLowerCase() || ''] || (value ? checkinStatusText(value) : '待签到')
}

function todayResultTagType(value: string | null | undefined): 'default' | 'success' | 'warning' | 'error' | 'info' {
  if (value === 'disabled' || value === 'not_checked' || !value) return 'default'
  if (value === 'pending') return 'warning'
  return checkinStatusTagType(value)
}

function itemStatusText(status: string): string {
  if (status === 'running') return '执行中'
  return todayResultText(status)
}

function itemTagType(status: string): 'default' | 'success' | 'warning' | 'error' | 'info' {
  return status === 'running' ? 'warning' : todayResultTagType(status)
}

function shortBatchId(id: string): string {
  return id.length > 12 ? `${id.slice(0, 8)}…` : id
}

function batchProgress(batch: BatchSummary): number {
  if (batch.total <= 0) return 100
  return Math.min(100, Math.round((batch.completed / batch.total) * 100))
}

function displayBatch(batch: BatchSummary): BatchSummary {
  return batchDetails.value[batch.batchId] || batch
}

function accountForPreview(id: string): WorkbenchAccount {
  const found = knownAccounts.value.find((account) => account.id === id)
  if (found) return found
  return {
    id,
    name: `账号 ${id.slice(0, 8)}`,
    siteType: '未知站点',
    baseUrl: '',
    enabled: true,
    retryEnabled: true,
    balanceWarning: false,
    todayResult: 'not_checked',
    selectable: true,
    skipReason: null,
    todayRuns: 0,
  }
}

function accountById(id: string): WorkbenchAccount | undefined {
  return knownAccounts.value.find((account) => account.id === id)
}

function isSelectable(account: WorkbenchAccount): boolean {
  return account.enabled !== false && account.selectable !== false
}

function canRetryAccount(id: string): boolean {
  const account = accountById(id)
  return account?.enabled !== false && account?.retryEnabled !== false
}

function rowKey(row: WorkbenchAccount): string {
  return row.id
}

function isSelected(id: string): boolean {
  return selectedIds.value.includes(id)
}

function setSelected(id: string, checked: boolean) {
  const next = new Set(selectedIds.value)
  if (checked) next.add(id)
  else next.delete(id)
  selectedIds.value = [...next]
  selectionTouched.value = true
}

function onCheckedRowKeys(keys: Array<string | number>) {
  selectedIds.value = keys.map(String)
  selectionTouched.value = true
}

function selectAllVisible() {
  selectedIds.value = selectableAccountIds(visibleAccounts.value)
  selectionTouched.value = true
}

function clearSelection() {
  selectedIds.value = []
  selectionTouched.value = true
}

function applyMetric(kind: 'success' | 'failed' | 'pending' | 'balanceWarning') {
  filters.todayResult = kind === 'balanceWarning' ? '' : kind
  filters.balanceWarning = kind === 'balanceWarning' ? 'true' : ''
  page.value = 1
  selectionTouched.value = false
}

function clearFilters() {
  Object.assign(filters, {
    keyword: '',
    siteType: '',
    userId: '',
    enabled: '',
    status: '',
    todayResult: '',
    balanceWarning: '',
  })
  page.value = 1
  selectionTouched.value = false
}

function focusAccount(id: string) {
  filters.keyword = accountById(id)?.name || id
  filters.balanceWarning = ''
  page.value = 1
  selectionTouched.value = false
}

async function loadWorkbench() {
  const seq = ++requestSeq
  loading.value = true
  loadError.value = ''
  try {
    const query = buildWorkbenchQuery(filters, { isAdmin: props.isAdmin, page: page.value, limit: PAGE_SIZE })
    const response = await request(`${apiUrl('/checkin-workbench')}?${query}`, {
      signal: requestSignal(CONTROL_TIMEOUT_MS),
      cache: 'no-store',
      headers: { 'Cache-Control': 'no-cache' },
    })
    const data = await responseData<WorkbenchResponse>(response)
    if (seq !== requestSeq) return
    workbench.value = data
    if (page.value > 1 && data.accounts.length === 0 && data.totalAccounts > 0) {
      page.value = 1
      selectionTouched.value = false
      void loadWorkbench()
      return
    }
    if (!selectionTouched.value) selectedIds.value = selectableAccountIds(data.accounts)
    else {
      const visible = new Set(data.accounts.map((account) => account.id))
      selectedIds.value = selectedIds.value.filter((id) => visible.has(id))
    }
    ensureTrackedBatches(data.activeBatches.map((batch) => batch.batchId))
    syncPollTimer()
  } catch (error) {
    if (seq !== requestSeq) return
    loadError.value = isTimeoutError(error)
      ? '工作台查询超时，请稍后重试。签到批次不会因页面查询超时而停止。'
      : (error instanceof Error ? error.message : '加载签到工作台失败')
    message.error(loadError.value)
  } finally {
    if (seq === requestSeq) loading.value = false
  }
}

function refreshWorkbench() {
  void loadWorkbench()
}

async function fetchBatchDetail(batchId: string, quiet = false) {
  if (!batchId) return
  if (!quiet) {
    detailLoading.value = true
    detailError.value = ''
  }
  const previous = batchDetails.value[batchId]?.status
  try {
    const response = await request(apiUrl(`/checkin-batches/${encodeURIComponent(batchId)}`), {
      cache: 'no-store',
      signal: requestSignal(BATCH_TIMEOUT_MS),
      headers: { 'Cache-Control': 'no-cache' },
    })
    const detail = await responseData<BatchDetail>(response)
    setBatchDetail(detail)
    rememberBatch(batchId)
    if (!quiet && activeDetailId.value === batchId) detailError.value = ''
    if (isTerminalBatchStatus(detail.status) && !isTerminalBatchStatus(previous) && !terminalRefreshQueued) {
      terminalRefreshQueued = true
      void loadWorkbench().finally(() => { terminalRefreshQueued = false })
    }
  } catch (error) {
    if (!quiet) {
      detailError.value = isTimeoutError(error)
        ? '批次查询超时。请稍后再次查询，后台签到不会停止。'
        : (error instanceof Error ? error.message : '查询批次详情失败')
    }
  } finally {
    if (!quiet) detailLoading.value = false
  }
}

async function pollTrackedBatches() {
  if (polling.value) return
  const ids = trackedBatchIds.value.filter((id) => !isTerminalBatchStatus(batchDetails.value[id]?.status))
  if (ids.length === 0) {
    syncPollTimer()
    return
  }
  polling.value = true
  try {
    await Promise.all(ids.map((id) => fetchBatchDetail(id, true)))
  } finally {
    polling.value = false
    syncPollTimer()
  }
}

function syncPollTimer() {
  const hasActive = trackedBatchIds.value.some((id) => !isTerminalBatchStatus(batchDetails.value[id]?.status))
  if (hasActive && !pollTimer) {
    pollTimer = setInterval(() => { void pollTrackedBatches() }, POLL_INTERVAL_MS)
  } else if (!hasActive && pollTimer) {
    clearInterval(pollTimer)
    pollTimer = null
  }
}

function openBatch(batchId: string) {
  activeDetailId.value = batchId
  detailDrawerOpen.value = true
  detailError.value = ''
  void fetchBatchDetail(batchId)
  syncPollTimer()
}

function refreshCurrentBatch() {
  if (activeDetailId.value) void fetchBatchDetail(activeDetailId.value)
}

async function resumeBatch() {
  const detail = currentBatchDetail.value
  if (!detail || !['pending', 'running'].includes(detail.status) || resumingBatch.value) return
  resumingBatch.value = true
  detailError.value = ''
  try {
    const response = await request(apiUrl(`/checkin-batches/${encodeURIComponent(detail.batchId)}/resume`), {
      method: 'POST',
      cache: 'no-store',
      signal: requestSignal(BATCH_TIMEOUT_MS),
      headers: { 'Cache-Control': 'no-cache' },
    })
    const resumed = await responseData<BatchDetail>(response)
    setBatchDetail(resumed)
    rememberBatch(resumed.batchId)
    message.success('批次已重新入队')
    syncPollTimer()
    void pollTrackedBatches()
  } catch (error) {
    detailError.value = isTimeoutError(error)
      ? '重新入队请求超时，请稍后查询批次状态。'
      : (error instanceof Error ? error.message : '批次重新入队失败')
  } finally {
    resumingBatch.value = false
  }
}

function createIdempotencyKey(): string {
  return `workbench-${Date.now()}-${Math.random().toString(36).slice(2)}`
}

function openPreview(ids: readonly string[], mode: PreviewMode) {
  const uniqueIds = [...new Set(ids.map((id) => id.trim()).filter(Boolean))]
  if (uniqueIds.length === 0) {
    message.warning('请先选择可执行账号')
    return
  }
  previewIds.value = uniqueIds
  previewMode.value = mode
  createError.value = ''
  previewOpen.value = true
}

function confirmPreview() {
  if (previewIds.value.length === 0 || creatingBatch.value) return
  const next: PendingCreate = {
    accountIds: [...previewIds.value],
    idempotencyKey: createIdempotencyKey(),
    createdAt: new Date().toISOString(),
  }
  pendingCreate.value = next
  persistPendingCreate()
  previewOpen.value = false
  void createBatch(next)
}

async function createBatch(payload: PendingCreate) {
  if (creatingBatch.value) return
  creatingBatch.value = true
  createError.value = ''
  pendingCreate.value = payload
  persistPendingCreate()
  try {
    const response = await request(apiUrl('/checkin-batches'), {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': payload.idempotencyKey,
      },
      body: JSON.stringify({ accountIds: payload.accountIds }),
      signal: requestSignal(BATCH_TIMEOUT_MS),
    })
    const detail = await responseData<BatchDetail>(response)
    setBatchDetail(detail)
    rememberBatch(detail.batchId)
    pendingCreate.value = null
    persistPendingCreate()
    createError.value = ''
    selectedIds.value = []
    selectionTouched.value = true
    activeDetailId.value = detail.batchId
    detailDrawerOpen.value = true
    message.success(`签到批次已创建，编号 ${shortBatchId(detail.batchId)}`)
    syncPollTimer()
    void pollTrackedBatches()
    await loadWorkbench()
  } catch (error) {
    createError.value = isTimeoutError(error)
      ? '创建请求超时。批次可能已经在后台创建，请使用同一个幂等标识重试，不要重新生成批次。'
      : (error instanceof Error ? error.message : '创建签到批次失败')
    message.error(createError.value)
  } finally {
    creatingBatch.value = false
  }
}

function retryPendingCreate() {
  if (pendingCreate.value) void createBatch(pendingCreate.value)
}

async function refreshBalance(account: WorkbenchAccount) {
  if (actionBusy.value || busyAccountIds.value.has(account.id)) return
  const next = new Set(busyAccountIds.value)
  next.add(account.id)
  busyAccountIds.value = next
  try {
    await request(apiUrl(`/accounts/${encodeURIComponent(account.id)}/refresh-balance`), {
      method: 'POST',
      signal: requestSignal(CONTROL_TIMEOUT_MS),
    })
    message.success(`已刷新 ${account.name} 的余额`)
    await loadWorkbench()
  } catch (error) {
    message.error(isTimeoutError(error) ? '余额刷新请求超时，请稍后重试' : (error instanceof Error ? error.message : '刷新余额失败'))
  } finally {
    const current = new Set(busyAccountIds.value)
    current.delete(account.id)
    busyAccountIds.value = current
  }
}

function changePage(nextPage: number) {
  if (nextPage === page.value || nextPage < 1 || nextPage > pageCount.value) return
  page.value = nextPage
  selectionTouched.value = false
  void loadWorkbench()
}

function openRuns(accountId: string, status = '', batchId = '') {
  emit('navigate-runs', {
    accountId,
    ...(status ? { status } : {}),
    ...(batchId ? { batchId } : {}),
  })
}

function openRunsForBatch() {
  const detail = currentBatchDetail.value
  if (detail) emit('navigate-runs', { batchId: detail.batchId })
}

function openAccounts(accountId?: string) {
  emit('navigate-accounts', accountId)
}

function openSettings() {
  emit('navigate-settings')
}

function handleOnline() {
  isOnline.value = true
  void loadWorkbench()
  void pollTrackedBatches()
}

function handleOffline() {
  isOnline.value = false
}

async function copyBatchId() {
  if (!currentBatchDetail.value) return
  try {
    await copyText(currentBatchDetail.value.batchId)
    message.success('批次编号已复制')
  } catch {
    message.error('复制失败，请手动选择批次编号')
  }
}

const queueColumns = computed<DataTableColumns<WorkbenchAccount>>(() => [
  {
    type: 'selection',
    disabled: (row: WorkbenchAccount) => !isSelectable(row) || actionBusy.value,
  },
  {
    title: '账号',
    key: 'name',
    minWidth: 220,
    render: (row) => h('div', { class: 'table-account-cell' }, [
      h('div', { class: 'table-account-name' }, [
        h('strong', row.name),
        row.balanceWarning ? h(NTag, { size: 'small', bordered: false, type: 'warning' }, { default: () => '余额预警' }) : null,
      ]),
      h('span', { class: 'muted table-account-url', title: row.baseUrl || '' }, row.baseUrl || '未设置地址'),
    ]),
  },
  {
    title: '今日结果',
    key: 'todayResult',
    width: 112,
    render: (row) => h(NTag, { size: 'small', bordered: false, type: todayResultTagType(row.todayResult) }, { default: () => todayResultText(row.todayResult) }),
  },
  {
    title: '余额',
    key: 'lastBalance',
    width: 100,
    render: (row) => h('span', { class: row.balanceWarning ? 'warning-text' : undefined }, formatBalance(row.lastBalance)),
  },
  { title: '今日尝试', key: 'todayRuns', width: 92, render: (row) => `${row.todayRuns} 次` },
  {
    title: '处理说明',
    key: 'skipReason',
    minWidth: 160,
    ellipsis: { tooltip: true },
    render: (row) => row.skipReason || (row.retryEnabled ? '可执行' : '重试已关闭'),
  },
  ...(props.isAdmin ? [{ title: '归属', key: 'ownerName', width: 100, render: (row: WorkbenchAccount) => row.ownerName || '未分配' }] : []),
  {
    title: '操作',
    key: 'actions',
    width: 248,
    render: (row) => h(NSpace, { size: 4 }, {
      default: () => [
        h(NButton, { size: 'tiny', type: 'primary', secondary: true, disabled: !isSelectable(row) || actionBusy.value, onClick: () => openPreview([row.id], 'checkin') }, { default: () => '签到' }),
        h(NButton, { size: 'tiny', secondary: true, loading: busyAccountIds.value.has(row.id), disabled: actionBusy.value, onClick: () => refreshBalance(row) }, { default: () => '刷新余额' }),
        h(NButton, { size: 'tiny', tertiary: true, disabled: actionBusy.value, onClick: () => openAccounts(row.id) }, { default: () => '编辑' }),
        h(NButton, { size: 'tiny', tertiary: true, disabled: actionBusy.value, onClick: () => openRuns(row.id) }, { default: () => '记录' }),
      ],
    }),
  },
])

watch(filters, () => {
  page.value = 1
  selectionTouched.value = false
  if (filterDebounce) clearTimeout(filterDebounce)
  filterDebounce = setTimeout(() => { void loadWorkbench() }, 240)
}, { deep: true })

watch(() => props.isAdmin, (isAdmin) => {
  if (!isAdmin) filters.userId = ''
  else void loadUsers()
})

onMounted(() => {
  restoreLocalState()
  if (props.isAdmin) void loadUsers()
  void loadWorkbench()
  void pollTrackedBatches()
  window.addEventListener('online', handleOnline)
  window.addEventListener('offline', handleOffline)
})

onUnmounted(() => {
  if (filterDebounce) clearTimeout(filterDebounce)
  if (pollTimer) clearInterval(pollTimer)
  window.removeEventListener('online', handleOnline)
  window.removeEventListener('offline', handleOffline)
})

</script>

<style scoped>
.workbench-panel {
  max-width: 1440px;
  margin: 0 auto;
  color: v-bind('themeVars.textColor1');
  font-variant-numeric: tabular-nums;
}

.workbench-header,
.section-heading,
.filters-heading,
.context-strip,
.header-actions,
.pending-create-content,
.error-content,
.queue-actions,
.card-heading,
.schedule-last,
.risk-main,
.failure-row,
.drawer-summary-head,
.drawer-progress-copy,
.account-card-top,
.account-card-meta,
.account-card-actions,
.preview-account-row,
.detail-item,
.detail-item-title {
  display: flex;
  align-items: center;
}

.workbench-header {
  justify-content: space-between;
  gap: 20px;
  padding: 4px 0 18px;
  border-bottom: 1px solid v-bind('themeVars.dividerColor');
}

.heading-block,
.header-actions,
.section-heading > div,
.filters-heading > div,
.failure-main,
.detail-item-main {
  min-width: 0;
}

.eyebrow {
  margin: 0 0 4px;
  color: v-bind('themeVars.primaryColor');
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.08em;
}

.workbench-title {
  margin: 0;
  font-size: clamp(24px, 3vw, 32px);
  line-height: 1.15;
  letter-spacing: -0.02em;
}

.workbench-subtitle {
  margin: 6px 0 0;
  color: v-bind('themeVars.textColor2');
  font-size: 13px;
}

.header-actions {
  justify-content: flex-end;
  gap: 12px;
  flex-wrap: wrap;
}

.business-date {
  display: grid;
  grid-template-columns: auto auto;
  gap: 2px 8px;
  align-items: baseline;
  font-size: 12px;
  text-align: right;
}

.business-date strong {
  font-size: 14px;
}

.timezone-label {
  grid-column: 2;
  color: v-bind('themeVars.textColor3');
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 11px;
}

.state-alert {
  margin-top: 12px;
}

.pending-create-content,
.error-content {
  justify-content: space-between;
  gap: 16px;
  flex-wrap: wrap;
}

.pending-create-content p {
  margin: 4px 0 0;
  color: v-bind('themeVars.textColor2');
}

.context-strip {
  gap: 22px;
  flex-wrap: wrap;
  min-height: 48px;
  padding: 12px 0 6px;
}

.context-item {
  display: flex;
  align-items: baseline;
  gap: 8px;
  font-size: 12px;
}

.context-item strong {
  font-size: 13px;
}

.schedule-context {
  margin-left: auto;
}

.metric-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 10px;
  margin: 8px 0 14px;
}

.metric-card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  min-height: 96px;
  padding: 15px 16px;
  border: 1px solid v-bind('themeVars.borderColor');
  border-radius: 10px;
  background: v-bind('themeVars.cardColor');
  color: v-bind('themeVars.textColor1');
  cursor: pointer;
  text-align: left;
  transition: border-color 200ms ease, background-color 200ms ease, transform 200ms ease;
}

.metric-card:hover,
.metric-card.active {
  border-color: v-bind('themeVars.primaryColor');
  background: color-mix(in srgb, v-bind('themeVars.primaryColor') 5%, v-bind('themeVars.cardColor'));
}

.metric-card:active {
  transform: translateY(1px);
}

.metric-card:focus-visible,
.batch-row:focus-visible {
  outline: 2px solid v-bind('themeVars.primaryColor');
  outline-offset: 2px;
}

.metric-label {
  color: v-bind('themeVars.textColor2');
  font-size: 13px;
}

.metric-value {
  margin-top: 4px;
  font-size: 28px;
  line-height: 1.1;
}

.metric-note {
  margin-top: auto;
  color: v-bind('themeVars.textColor3');
  font-size: 11px;
}

.metric-success .metric-value,
.success-number {
  color: v-bind('themeVars.successColor');
}

.metric-danger .metric-value,
.danger-number {
  color: v-bind('themeVars.errorColor');
}

.metric-pending .metric-value {
  color: v-bind('themeVars.warningColor');
}

.metric-warning .metric-value,
.warning-number,
.warning-text {
  color: v-bind('themeVars.warningColor');
}

.filters-card {
  margin-bottom: 14px;
}

.filters-heading {
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 12px;
}

.filters-heading > div {
  display: flex;
  align-items: baseline;
  gap: 10px;
}

.filters-heading .muted {
  font-size: 12px;
}

.filters-grid {
  display: grid;
  grid-template-columns: minmax(220px, 1.6fr) repeat(5, minmax(120px, 1fr));
  gap: 8px;
}

.dashboard-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(290px, 340px);
  gap: 14px;
  align-items: start;
}

.queue-panel,
.sidebar-stack {
  min-width: 0;
}

.queue-panel {
  padding: 16px;
  border: 1px solid v-bind('themeVars.borderColor');
  border-radius: 10px;
  background: v-bind('themeVars.cardColor');
}

.section-heading {
  justify-content: space-between;
  gap: 14px;
  margin-bottom: 12px;
}

.section-heading h3 {
  margin: 0;
  font-size: 17px;
  line-height: 1.3;
}

.section-heading p {
  margin: 3px 0 0;
  font-size: 12px;
}

.queue-actions {
  justify-content: flex-end;
  gap: 6px;
  flex-wrap: wrap;
}

.queue-hint {
  margin-bottom: 10px;
}

.queue-table {
  margin-top: 4px;
}

.table-account-cell {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}

.table-account-name {
  display: flex;
  align-items: center;
  gap: 6px;
}

.table-account-url {
  overflow: hidden;
  max-width: 260px;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 11px;
}

.mobile-queue,
.mobile-action-bar {
  display: none;
}

.queue-empty {
  padding: 32px 0;
}

.pagination-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-top: 14px;
}

.sidebar-stack {
  display: grid;
  gap: 12px;
}

.card-heading {
  justify-content: space-between;
  gap: 8px;
  width: 100%;
  font-size: 14px;
  font-weight: 700;
}

.schedule-main {
  display: flex;
  flex-direction: column;
  gap: 5px;
  padding: 2px 0 12px;
}

.schedule-main strong {
  font-size: 19px;
}

.schedule-cron {
  margin: 0;
  padding: 9px 0;
  border-top: 1px solid v-bind('themeVars.dividerColor');
  border-bottom: 1px solid v-bind('themeVars.dividerColor');
  color: v-bind('themeVars.textColor2');
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 11px;
  line-height: 1.5;
  overflow-wrap: anywhere;
}

.schedule-last {
  justify-content: space-between;
  align-items: baseline;
  gap: 8px;
  padding-top: 10px;
  font-size: 12px;
}

.settings-link {
  margin-top: 10px;
  padding-left: 0;
}

.batch-list,
.risk-list,
.failure-list {
  display: grid;
}

.batch-row {
  display: block;
  width: 100%;
  padding: 10px 0;
  border: 0;
  border-bottom: 1px solid v-bind('themeVars.dividerColor');
  background: transparent;
  color: v-bind('themeVars.textColor1');
  cursor: pointer;
  text-align: left;
  transition: background-color 180ms ease;
}

.batch-row:last-child {
  border-bottom: 0;
}

.batch-row:hover {
  background: color-mix(in srgb, v-bind('themeVars.primaryColor') 5%, transparent);
}

.batch-row-top,
.batch-row-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.batch-row-top {
  font-size: 12px;
}

.batch-row-meta {
  margin: 6px 0;
  color: v-bind('themeVars.textColor3');
  font-size: 11px;
}

.compact-list .batch-row {
  padding: 8px 0;
}

.risk-row,
.failure-row {
  justify-content: space-between;
  gap: 8px;
  padding: 9px 0;
  border-bottom: 1px solid v-bind('themeVars.dividerColor');
}

.risk-row:last-child,
.failure-row:last-child {
  border-bottom: 0;
}

.risk-main {
  justify-content: space-between;
  gap: 8px;
  flex: 1;
  min-width: 0;
  font-size: 12px;
}

.risk-main strong,
.failure-main strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.failure-row {
  align-items: flex-start;
}

.failure-main {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.failure-main > span {
  font-size: 11px;
}

.failure-main p {
  margin: 2px 0 0;
  color: v-bind('themeVars.textColor2');
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.warning-card {
  border-color: color-mix(in srgb, v-bind('themeVars.warningColor') 32%, v-bind('themeVars.borderColor'));
}

.loading-shell {
  display: grid;
  gap: 14px;
  padding-top: 14px;
}

.skeleton-heading {
  width: min(340px, 70%);
}

.skeleton-metrics,
.skeleton-columns {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 10px;
}

.skeleton-columns {
  grid-template-columns: minmax(0, 1fr) 340px;
}

.account-card {
  padding: 13px 0;
  border-bottom: 1px solid v-bind('themeVars.dividerColor');
}

.account-card:first-child {
  border-top: 1px solid v-bind('themeVars.dividerColor');
}

.account-card-top {
  gap: 8px;
}

.account-card-title {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1;
  gap: 2px;
  font-size: 13px;
}

.account-card-title strong,
.account-url {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.account-card-title .muted,
.account-card-meta {
  font-size: 11px;
}

.account-card-meta {
  gap: 10px;
  margin: 9px 0 0 30px;
  color: v-bind('themeVars.textColor2');
}

.account-url {
  min-width: 0;
  flex: 1;
}

.skip-reason {
  margin: 8px 0 0 30px;
  color: v-bind('themeVars.warningColor');
  font-size: 12px;
}

.account-card-actions {
  justify-content: flex-end;
  gap: 6px;
  margin: 10px 0 0 30px;
}

.mobile-action-bar {
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 10px 12px;
  margin-top: 12px;
  border: 1px solid v-bind('themeVars.borderColor');
  border-radius: 10px;
  background: color-mix(in srgb, v-bind('themeVars.cardColor') 94%, transparent);
  box-shadow: 0 4px 14px color-mix(in srgb, v-bind('themeVars.textColor3') 12%, transparent);
  font-size: 12px;
}

.preview-intro strong {
  font-size: 15px;
}

.preview-intro p {
  margin: 5px 0 0;
  color: v-bind('themeVars.textColor2');
  font-size: 13px;
  line-height: 1.6;
}

.preview-summary-grid,
.drawer-stats {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 8px;
  margin: 16px 0 12px;
}

.preview-summary-grid > div,
.drawer-stats > div {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px;
  border-radius: 8px;
  background: color-mix(in srgb, v-bind('themeVars.primaryColor') 5%, v-bind('themeVars.cardColor'));
  font-size: 12px;
}

.preview-summary-grid strong {
  font-size: 20px;
}

.preview-note {
  margin-bottom: 12px;
}

.preview-account-list {
  max-height: 260px;
  overflow-y: auto;
}

.preview-account-row {
  justify-content: space-between;
  gap: 10px;
  padding: 9px 0;
  border-bottom: 1px solid v-bind('themeVars.dividerColor');
}

.preview-account-row > div {
  display: flex;
  flex-direction: column;
  min-width: 0;
  gap: 3px;
}

.preview-account-row .muted {
  font-size: 11px;
}

.drawer-loading {
  display: grid;
  gap: 16px;
}

.drawer-alert {
  margin-bottom: 12px;
}

.drawer-summary-head {
  justify-content: space-between;
  align-items: flex-start;
  gap: 12px;
}

.batch-id {
  display: block;
  max-width: 290px;
  margin: 8px 0 0;
  padding: 0;
  border: 0;
  background: transparent;
  color: v-bind('themeVars.textColor3');
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: copy;
  text-align: left;
}

.batch-id:focus-visible {
  outline: 2px solid v-bind('themeVars.primaryColor');
  outline-offset: 2px;
}

.drawer-progress-copy {
  justify-content: space-between;
  gap: 8px;
  margin: 16px 0 7px;
  color: v-bind('themeVars.textColor2');
  font-size: 12px;
}

.drawer-stats {
  grid-template-columns: repeat(4, minmax(0, 1fr));
  margin: 14px 0;
}

.drawer-stats strong {
  font-size: 18px;
}

.detail-list {
  margin-top: 8px;
}

.detail-item {
  justify-content: space-between;
  align-items: flex-start;
  gap: 10px;
  width: 100%;
}

.detail-item-title {
  gap: 8px;
  flex-wrap: wrap;
}

.detail-item-main p {
  margin: 5px 0 3px;
  color: v-bind('themeVars.textColor2');
  font-size: 12px;
  overflow-wrap: anywhere;
}

.detail-item-main > span {
  font-size: 11px;
}

.muted {
  color: v-bind('themeVars.textColor3');
}

@media (max-width: 1100px) {
  .filters-grid {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  .filters-grid > :first-child {
    grid-column: span 3;
  }
}

@media (max-width: 880px) {
  .dashboard-grid {
    grid-template-columns: minmax(0, 1fr);
  }

  .sidebar-stack {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .schedule-card {
    grid-column: span 2;
  }
}

@media (max-width: 720px) {
  .workbench-header {
    align-items: flex-start;
    flex-direction: column;
    gap: 12px;
  }

  .header-actions {
    justify-content: space-between;
    width: 100%;
  }

  .business-date {
    text-align: left;
  }

  .schedule-context {
    margin-left: 0;
  }

  .metric-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .filters-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .filters-grid > :first-child {
    grid-column: span 2;
  }

  .filters-heading > div {
    align-items: flex-start;
    flex-direction: column;
    gap: 3px;
  }

  .queue-panel {
    padding: 12px;
  }

  .queue-heading {
    align-items: flex-start;
    flex-direction: column;
  }

  .queue-actions {
    justify-content: flex-start;
    width: 100%;
  }

  .desktop-queue {
    display: none;
  }

  .mobile-queue,
  .mobile-action-bar {
    display: block;
  }

  .mobile-action-bar {
    position: sticky;
    bottom: 10px;
    z-index: 1;
    display: flex;
  }

  .sidebar-stack {
    grid-template-columns: minmax(0, 1fr);
  }

  .schedule-card {
    grid-column: auto;
  }

  .skeleton-metrics,
  .skeleton-columns {
    grid-template-columns: minmax(0, 1fr);
  }

  .preview-summary-grid,
  .drawer-stats {
    gap: 6px;
  }

  .drawer-summary-head {
    flex-direction: column;
  }
}

@media (prefers-reduced-motion: reduce) {
  .metric-card,
  .batch-row {
    transition: none;
  }

  .metric-card:active {
    transform: none;
  }
}
</style>
