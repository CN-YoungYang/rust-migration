<template>
  <div ref="panelRoot" class="panel">
    <div class="panel-header">
      <div>
        <h2 class="panel-title">数据统计</h2>
        <n-text depth="3" class="panel-subtitle">{{ resultUserName }} · {{ resultRangeLabel }}</n-text>
      </div>
      <n-space align="center" :size="8" wrap>
        <n-select
          v-if="isAdmin"
          v-model:value="selectedUserId"
          :options="userOptions"
          placeholder="全部用户"
          :loading="usersLoading"
          size="small"
          style="width: 140px"
        />
        <n-date-picker
          v-model:value="dateRange"
          type="daterange"
          :disabled="loading"
          clearable
          size="small"
          style="width: 260px"
          :is-date-disabled="dateDisabled"
        />
        <n-button type="primary" size="small" :loading="loading" :disabled="loading || dateRangeInvalid || dateRangeTooLong" @click="loadStatistics">
          {{ loading ? '查询中…' : '查询' }}
        </n-button>
        <n-button v-for="days in [7, 30, 90]" :key="days" size="small" :type="isActiveRange(days) ? 'primary' : 'default'" :disabled="loading" :aria-pressed="isActiveRange(days)" @click="applyRange(days)">
          {{ days }}天
        </n-button>
      </n-space>
    </div>

    <n-alert v-if="dateValidationMessage" type="error" :show-icon="true" class="date-error" role="alert">
      {{ dateValidationMessage }}
    </n-alert>

    <div v-if="loading && !statistics" class="initial-loading" role="status" aria-live="polite">
      <n-spin size="large" />
      <p class="muted">正在加载统计数据…</p>
    </div>

    <n-alert v-if="loadError" type="error" :show-icon="true" class="load-error" role="alert" :action="() => h(NButton, { size: 'small', onClick: loadStatistics }, { default: () => '重新查询' })">
      {{ loadError }}
    </n-alert>

    <div v-if="statistics" class="stats-content" :aria-busy="loading">
      <n-text v-if="loading" depth="3" class="refresh-status" role="status" aria-live="polite">
        正在更新，当前仍显示上次结果。
      </n-text>

      <n-grid class="summary-grid" :cols="4" :x-gap="12" :y-gap="12" responsive="screen" item-responsive>
        <n-grid-item>
          <n-statistic label="今日执行">
            <span class="stat-value">{{ statistics.overview.todayTotal }}<small class="stat-unit">次</small></span>
            <div class="stat-desc">成功 {{ statistics.overview.todaySuccess }} · 已签 {{ statistics.overview.todayAlreadyChecked }} · 等待 {{ statistics.overview.todayPending }} · 失败 {{ statistics.overview.todayFailed }}</div>
          </n-statistic>
        </n-grid-item>
        <n-grid-item>
          <n-statistic label="区间成功率">
            <span class="stat-value" :style="statistics.overview.todayFailed > 0 ? { color: themeVars.errorColor } : undefined">
              {{ statistics.overview.completedRuns > 0 ? statistics.overview.successRate.toFixed(1) : '—' }}<small v-if="statistics.overview.completedRuns > 0" class="stat-unit">%</small>
            </span>
            <div class="stat-desc">按已完成记录计算 · 总执行 {{ statistics.overview.totalRuns }} 次</div>
          </n-statistic>
        </n-grid-item>
        <n-grid-item>
          <n-statistic label="启用账户">
            <span class="stat-value">{{ statistics.overview.enabledAccounts }}<small class="stat-unit">/ {{ statistics.overview.totalAccounts }}</small></span>
            <div class="stat-desc">启用率 {{ enabledRatio }}%</div>
          </n-statistic>
        </n-grid-item>
        <n-grid-item>
          <n-statistic label="当前总余额">
            <span class="stat-value"><small class="stat-unit">$</small>{{ statistics.overview.totalBalance.toFixed(2) }}</span>
            <div class="stat-desc">基于账户最后刷新结果</div>
          </n-statistic>
        </n-grid-item>
      </n-grid>

      <n-card class="chart-card" :bordered="true">
        <template #header>
          <div class="chart-heading">
            <div>
              <h3 class="section-title">每日执行量</h3>
              <p class="muted section-desc">统一按区间内最大单日执行量绘制，柱高可直接跨日期比较。</p>
            </div>
            <div class="chart-scale muted">
              <span>最高单日</span>
              <strong>{{ maxDailyTotal }} 次</strong>
            </div>
          </div>
        </template>

        <div v-if="statistics.dailyTrend.length > 0" role="group" aria-label="每日签到趋势图">
          <div class="chart-legend">
            <span class="legend-item"><span class="dot" :style="{ background: themeVars.successColor }"></span>成功</span>
            <span class="legend-item"><span class="dot" :style="{ background: themeVars.primaryColor }"></span>已签到</span>
            <span class="legend-item"><span class="dot" :style="{ background: themeVars.warningColor }"></span>等待中</span>
            <span class="legend-item"><span class="dot" :style="{ background: themeVars.errorColor }"></span>失败</span>
          </div>
          <div class="plot-scroll" aria-label="每日执行量图表，可横向滚动">
            <div class="bar-chart" role="group" aria-label="每日签到趋势数据">
              <button
                v-for="day in dailyTrendSeries"
                :key="day.date"
                type="button"
                class="bar-group"
                :class="{ selected: selectedTrendDay?.date === day.date }"
                :data-trend-date="day.date"
                :tabindex="selectedTrendDay?.date === day.date ? 0 : -1"
                :aria-label="trendAriaLabel(day)"
                :aria-pressed="selectedTrendDay?.date === day.date"
                @focus="activeTrendDate = day.date"
                @click="activeTrendDate = day.date"
                @keydown.left.prevent="moveTrendFocus(day.date, -1)"
                @keydown.right.prevent="moveTrendFocus(day.date, 1)"
                @keydown.home.prevent="moveTrendFocus(day.date, -dailyTrendSeries.length)"
                @keydown.end.prevent="moveTrendFocus(day.date, dailyTrendSeries.length)"
              >
                <div class="bar-stack" aria-hidden="true">
                  <div v-if="day.success > 0" class="bar" :style="{ height: getTrendHeight(day.success) + '%', background: themeVars.successColor }"></div>
                  <div v-if="day.alreadyChecked > 0" class="bar" :style="{ height: getTrendHeight(day.alreadyChecked) + '%', background: themeVars.primaryColor }"></div>
                  <div v-if="day.pending > 0" class="bar" :style="{ height: getTrendHeight(day.pending) + '%', background: themeVars.warningColor }"></div>
                  <div v-if="day.failed > 0" class="bar" :style="{ height: getTrendHeight(day.failed) + '%', background: themeVars.errorColor }"></div>
                </div>
                <span class="bar-label">{{ formatDate(day.date) }}</span>
                <span class="bar-value">{{ day.total }}</span>
                <span class="bar-rate">{{ completedCount(day) > 0 ? `${day.successRate.toFixed(0)}%` : '—' }}</span>
              </button>
            </div>
          </div>
          <n-descriptions v-if="selectedTrendDay" :title="selectedTrendDay.date" :column="2" size="small" bordered class="trend-detail">
            <n-descriptions-item label="总执行">{{ selectedTrendDay.total }}</n-descriptions-item>
            <n-descriptions-item label="成功率">{{ completedCount(selectedTrendDay) > 0 ? `${selectedTrendDay.successRate.toFixed(1)}%` : '—' }}</n-descriptions-item>
            <n-descriptions-item label="成功">{{ selectedTrendDay.success }}</n-descriptions-item>
            <n-descriptions-item label="已签到">{{ selectedTrendDay.alreadyChecked }}</n-descriptions-item>
            <n-descriptions-item label="等待中">{{ selectedTrendDay.pending }}</n-descriptions-item>
            <n-descriptions-item label="失败">{{ selectedTrendDay.failed }}</n-descriptions-item>
          </n-descriptions>
        </div>
        <n-empty v-else description="所选时间范围内无签到记录">
          <template #extra>
            <n-button size="small" @click="applyRange(30)" :disabled="loading">查看最近30天</n-button>
          </template>
        </n-empty>
      </n-card>

      <n-card class="table-card" :bordered="true">
        <template #header>
          <h3 class="section-title">站点统计</h3>
        </template>
        <n-data-table
          :columns="siteColumns"
          :data="statistics.siteStats"
          :scroll-x="900"
        >
          <template #empty>暂无站点统计</template>
        </n-data-table>
      </n-card>

      <n-card class="failure-card" :bordered="true">
        <template #header>
          <div class="chart-heading">
            <div>
              <h3 class="section-title">异常与风险</h3>
              <p class="muted section-desc">最近失败不受当前日期范围限制，用于快速定位仍需处理的问题。</p>
            </div>
            <div class="risk-summary muted">
              <span>风险站点</span>
              <strong>{{ highestRiskSite }}</strong>
            </div>
          </div>
        </template>
        <n-list v-if="statistics.recentFailures.length > 0" class="failure-list">
          <n-list-item v-for="failure in statistics.recentFailures" :key="failure.runId">
            <div class="failure-item">
              <div class="failure-main">
                <div class="failure-title">
                  <strong>{{ failure.accountName }}</strong>
                  <n-tag size="small" :bordered="false">{{ failure.siteType }}</n-tag>
                  <n-tag v-if="failure.ownerName" size="small" :bordered="false" type="info">{{ failure.ownerName }}</n-tag>
                </div>
                <p class="failure-message" :title="failure.message || ''">
                  {{ failure.message || '无错误消息' }}
                </p>
                <p class="muted failure-time">{{ formatDateTimeFull(failure.createdAt) }}</p>
              </div>
              <n-button size="small" secondary @click="copyFailureSummary(failure)">复制摘要</n-button>
            </div>
          </n-list-item>
        </n-list>
        <n-empty v-else description="暂无失败记录" />
      </n-card>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, h, ref, nextTick, onMounted, watch } from 'vue'
import {
  NAlert,
  NButton,
  NCard,
  NDataTable,
  NDatePicker,
  NDescriptions,
  NDescriptionsItem,
  NEmpty,
  NGrid,
  NGridItem,
  NList,
  NListItem,
  NSelect,
  NSpace,
  NSpin,
  NStatistic,
  NTag,
  NText,
  useMessage,
  useThemeVars,
  type DataTableColumns,
} from 'naive-ui'
import { apiUrl, request, responseData } from '../utils/api'
import { formatDateTimeFull, formatDateInput } from '../utils/format'
import { copyText } from '../utils/clipboard'
import type { CurrentUser } from '../types'
import { useUsers } from '../composables/useUsers'

interface Statistics {
  overview: {
    totalAccounts: number
    enabledAccounts: number
    todaySuccess: number
    todayAlreadyChecked: number
    todayFailed: number
    todayPending: number
    todayTotal: number
    totalRuns: number
    completedRuns: number
    successRate: number
    totalBalance: number
  }
  dailyTrend: Array<{
    date: string
    success: number
    failed: number
    alreadyChecked: number
    pending: number
    total: number
    successRate: number
  }>
  siteStats: Array<{
    siteType: string
    accountCount: number
    totalRuns: number
    success: number
    alreadyChecked: number
    failed: number
    pending: number
    successRate: number
    avgDuration: number | null
  }>
  recentFailures: Array<{
    runId: string
    accountId: string
    accountName: string
    siteType: string
    ownerName?: string | null
    message?: string | null
    createdAt: string
  }>
}

const props = defineProps<{
  currentUser: CurrentUser | null
  isAdmin: boolean
}>()

const message = useMessage()
const themeVars = useThemeVars()

const { allUsers, usersLoading, loadUsers } = useUsers(() => props.isAdmin)
const loading = ref(false)
const statistics = ref<Statistics | null>(null)
const startDate = ref('')
const endDate = ref('')
const selectedUserId = ref('')
const activeTrendDate = ref('')
const loadError = ref('')
const panelRoot = ref<HTMLElement | null>(null)
const appliedQuery = ref({ startDate: '', endDate: '', userName: '' })
let requestSeq = 0

const userOptions = computed(() => {
  const options: Array<{ label: string; value: string }> = [{ label: '全部用户', value: '' }]
  for (const user of allUsers.value) {
    options.push({
      label: user.id === props.currentUser?.id ? `${user.username}（我）` : user.username,
      value: user.id,
    })
  }
  return options
})

const dateRange = computed<[number, number] | null>({
  get: (): [number, number] | null => {
    if (!startDate.value || !endDate.value) return null
    return [new Date(`${startDate.value}T00:00:00`).getTime(), new Date(`${endDate.value}T00:00:00`).getTime()] as [number, number]
  },
  set: (value: [number, number] | null) => {
    if (!value) {
      startDate.value = ''
      endDate.value = ''
      return
    }
    startDate.value = formatDateInput(new Date(value[0]))
    endDate.value = formatDateInput(new Date(value[1]))
  },
})

const selectedUserName = computed(() => {
  if (!props.isAdmin) return props.currentUser?.username ? `${props.currentUser.username}（我的数据）` : '我的数据'
  if (!selectedUserId.value) return '全部用户'
  return allUsers.value.find((user) => user.id === selectedUserId.value)?.username || '指定用户'
})

const inputRangeLabel = computed(() => {
  if (!startDate.value || !endDate.value) return '默认时间范围'
  return `${startDate.value} 至 ${endDate.value}`
})

const resultUserName = computed(() => (statistics.value ? appliedQuery.value.userName : selectedUserName.value))
const resultRangeLabel = computed(() => {
  if (!statistics.value) return inputRangeLabel.value
  return `${appliedQuery.value.startDate} 至 ${appliedQuery.value.endDate}`
})

const dateRangeInvalid = computed(() => {
  return Boolean(startDate.value && endDate.value && startDate.value > endDate.value)
})

const dateRangeTooLong = computed(() => {
  if (!startDate.value || !endDate.value || dateRangeInvalid.value) return false
  return inclusiveDayCount(startDate.value, endDate.value) > 180
})

const dateValidationMessage = computed(() => {
  if (dateRangeInvalid.value) return '开始日期不能晚于结束日期。'
  if (dateRangeTooLong.value) return '统计查询范围不能超过180天。'
  return ''
})

function dateDisabled(ts: number): boolean {
  if (!dateRange.value) return false
  return ts < dateRange.value[0] || ts > dateRange.value[1]
}

const enabledRatio = computed(() => {
  if (!statistics.value || statistics.value.overview.totalAccounts === 0) return '0.0'
  return ((statistics.value.overview.enabledAccounts / statistics.value.overview.totalAccounts) * 100).toFixed(1)
})

const maxDailyTotal = computed(() => {
  return Math.max(...dailyTrendSeries.value.map((day) => day.total), 0)
})

const dailyTrendSeries = computed<Statistics['dailyTrend']>(() => {
  if (!statistics.value || !appliedQuery.value.startDate || !appliedQuery.value.endDate) return []
  const byDate = new Map(statistics.value.dailyTrend.map((day) => [day.date, day]))
  const series: Statistics['dailyTrend'] = []
  const cursor = new Date(`${appliedQuery.value.startDate}T00:00:00`)
  const end = new Date(`${appliedQuery.value.endDate}T00:00:00`)

  while (cursor <= end) {
    const date = formatDateInput(cursor)
    series.push(byDate.get(date) || {
      date,
      success: 0,
      failed: 0,
      alreadyChecked: 0,
      pending: 0,
      total: 0,
      successRate: 0,
    })
    cursor.setDate(cursor.getDate() + 1)
  }
  // 倒序：最新日期置于最左（索引 0），让首页/概览先看到当天数据。
  return series.reverse()
})

const selectedTrendDay = computed(() => {
  const selected = dailyTrendSeries.value.find((day) => day.date === activeTrendDate.value)
  if (selected) return selected
  // 倒序数组：索引 0 即最新，从最新往旧找首个有数据的一天作为默认选中。
  return dailyTrendSeries.value.find((day) => day.total > 0) || dailyTrendSeries.value[0] || null
})

const highestRiskSite = computed(() => {
  if (!statistics.value || statistics.value.siteStats.length === 0) return '无'
  const failedSites = statistics.value.siteStats
    .filter((site) => site.failed > 0)
    .sort((a, b) => b.failed - a.failed || a.successRate - b.successRate)
  return failedSites[0]?.siteType || '无'
})

// 设置默认时间范围（最近30天）
function setDefaultRange() {
  const today = new Date()
  const thirtyDaysAgo = new Date(today)
  thirtyDaysAgo.setDate(today.getDate() - 29)

  endDate.value = formatDateInput(today)
  startDate.value = formatDateInput(thirtyDaysAgo)
}

function applyRange(days: number) {
  const today = new Date()
  const start = new Date(today)
  start.setDate(today.getDate() - (days - 1))
  endDate.value = formatDateInput(today)
  startDate.value = formatDateInput(start)
  void loadStatistics()
}

function isActiveRange(days: number): boolean {
  if (!startDate.value || !endDate.value) return false
  const start = new Date(`${startDate.value}T00:00:00`)
  const end = new Date(`${endDate.value}T00:00:00`)
  const today = formatDateInput(new Date())
  return endDate.value === today && Math.round((end.getTime() - start.getTime()) / 86400000) + 1 === days
}

function inclusiveDayCount(start: string, end: string): number {
  const startDateTime = new Date(`${start}T00:00:00`)
  const endDateTime = new Date(`${end}T00:00:00`)
  return Math.round((endDateTime.getTime() - startDateTime.getTime()) / 86400000) + 1
}

function formatDate(dateStr: string): string {
  const [year, month, day] = dateStr.split('-')
  const crossesYear = appliedQuery.value.startDate.slice(0, 4) !== appliedQuery.value.endDate.slice(0, 4)
  return crossesYear ? `${year}/${month}/${day}` : `${month}/${day}`
}

function getTrendHeight(value: number): number {
  if (maxDailyTotal.value === 0) return 0
  return (value / maxDailyTotal.value) * 100
}

function completedCount(item: { success: number; alreadyChecked: number; failed: number }): number {
  return item.success + item.alreadyChecked + item.failed
}

function moveTrendFocus(date: string, offset: number) {
  const currentIndex = dailyTrendSeries.value.findIndex((day) => day.date === date)
  if (currentIndex < 0) return
  const nextIndex = Math.max(0, Math.min(dailyTrendSeries.value.length - 1, currentIndex + offset))
  const nextDate = dailyTrendSeries.value[nextIndex]?.date
  if (!nextDate) return
  activeTrendDate.value = nextDate
  void nextTick(() => {
    panelRoot.value?.querySelector<HTMLElement>(`[data-trend-date="${nextDate}"]`)?.focus()
  })
}

function trendAriaLabel(day: Statistics['dailyTrend'][number]): string {
  const rate = completedCount(day) > 0 ? `${day.successRate.toFixed(0)}%` : '无样本'
  return `${formatDate(day.date)}：总计 ${day.total} 次，成功 ${day.success} 次，已签到 ${day.alreadyChecked} 次，等待中 ${day.pending} 次，失败 ${day.failed} 次，成功率 ${rate}`
}

function rateTagType(rate: number, hasSamples: boolean): 'default' | 'success' | 'info' | 'warning' | 'error' {
  if (!hasSamples) return 'default'
  if (rate >= 90) return 'success'
  if (rate >= 70) return 'info'
  if (rate >= 50) return 'warning'
  return 'error'
}

const siteColumns = computed<DataTableColumns<Statistics['siteStats'][number]>>(() => [
  {
    title: '站点类型',
    key: 'siteType',
    render: (site) => h(NTag, { size: 'small', bordered: false }, { default: () => site.siteType }),
  },
  { title: '账户数', key: 'accountCount' },
  { title: '总签到', key: 'totalRuns' },
  { title: '成功', key: 'success' },
  { title: '已签到', key: 'alreadyChecked' },
  { title: '失败', key: 'failed' },
  { title: '等待中', key: 'pending' },
  {
    title: '成功率',
    key: 'successRate',
    render: (site) => {
      const hasSamples = completedCount(site) > 0
      return h(
        NTag,
        { size: 'small', bordered: false, type: rateTagType(site.successRate, hasSamples) },
        { default: () => (hasSamples ? `${site.successRate.toFixed(1)}%` : '—') },
      )
    },
  },
  {
    title: '平均耗时',
    key: 'avgDuration',
    render: (site) => (site.avgDuration == null ? '—' : `${site.avgDuration.toFixed(0)}ms`),
  },
])

async function loadStatistics() {
  if (dateRangeInvalid.value) {
    message.error('开始日期不能晚于结束日期')
    return
  }
  if (dateRangeTooLong.value) {
    message.error('统计查询范围不能超过180天')
    return
  }
  const requestedQuery = {
    startDate: startDate.value,
    endDate: endDate.value,
    userName: selectedUserName.value,
  }
  const seq = ++requestSeq
  loadError.value = ''
  loading.value = true
  try {
    const params = new URLSearchParams()
    if (startDate.value) params.append('startDate', startDate.value)
    if (endDate.value) params.append('endDate', endDate.value)
    if (props.isAdmin && selectedUserId.value) params.append('userId', selectedUserId.value)

    const url = apiUrl(`/statistics?${params.toString()}`)
    const response = await request(url)
    const data = await responseData<Statistics>(response)
    if (seq === requestSeq) {
      statistics.value = data
      appliedQuery.value = requestedQuery
      activeTrendDate.value = data.dailyTrend.at(-1)?.date || requestedQuery.endDate
    }
  } catch (error) {
    if (seq === requestSeq) {
      loadError.value = error instanceof Error ? error.message : '加载统计数据失败'
      message.error(loadError.value)
    }
  } finally {
    if (seq === requestSeq) {
      loading.value = false
    }
  }
}

async function copyFailureSummary(failure: Statistics['recentFailures'][number]) {
  const summary = [
    `账户: ${failure.accountName}`,
    `站点: ${failure.siteType}`,
    `归属: ${failure.ownerName || '-'}`,
    `时间: ${formatDateTimeFull(failure.createdAt)}`,
    `消息: ${failure.message || '-'}`,
  ].join('\n')

  try {
    await copyText(summary)
    message.success('失败摘要已复制')
  } catch {
    message.error('复制失败，请手动选择消息内容')
  }
}

onMounted(() => {
  setDefaultRange()
  if (props.isAdmin) {
    void loadUsers()
  }
  void loadStatistics()
})

watch(selectedUserId, () => {
  void loadStatistics()
})

watch(() => props.isAdmin, (isAdmin) => {
  if (isAdmin) {
    void loadUsers()
    return
  }
  selectedUserId.value = ''
})
</script>

<style scoped>
.panel-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
  margin-bottom: 14px;
}

.panel-title {
  margin: 0;
  font-size: 18px;
  font-weight: 700;
  line-height: 1.3;
}

.panel-subtitle {
  display: block;
  margin-top: 2px;
  font-size: 13px;
}

.date-error {
  margin-bottom: 12px;
}

.initial-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  padding: 48px 0;
}

.load-error {
  margin-bottom: 12px;
}

.stats-content {
  position: relative;
}

.refresh-status {
  display: block;
  margin-bottom: 10px;
  font-size: 13px;
}

.summary-grid {
  margin-bottom: 14px;
}

.stat-value {
  font-size: 24px;
  font-weight: 600;
  line-height: 1.2;
}

.stat-unit {
  font-size: 13px;
  font-weight: 400;
  margin-left: 2px;
}

.stat-desc {
  margin-top: 4px;
  font-size: 12px;
  color: v-bind('themeVars.textColor3');
}

.chart-card,
.table-card,
.failure-card {
  margin-bottom: 14px;
}

.chart-heading {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  width: 100%;
}

.section-title {
  margin: 0;
  font-size: 15px;
  font-weight: 700;
  line-height: 1.3;
}

.section-desc {
  margin: 2px 0 0;
  font-size: 12px;
}

.chart-scale,
.risk-summary {
  display: flex;
  align-items: baseline;
  gap: 8px;
  font-size: 13px;
  white-space: nowrap;
}

.chart-legend {
  display: flex;
  gap: 16px;
  flex-wrap: wrap;
  margin-bottom: 10px;
}

.legend-item {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
}

.dot {
  width: 10px;
  height: 10px;
  border-radius: 3px;
  display: inline-block;
}

.plot-scroll {
  overflow-x: auto;
  padding-bottom: 4px;
}

.bar-chart {
  display: flex;
  align-items: flex-end;
  gap: 4px;
  min-width: max-content;
}

.bar-group {
  display: flex;
  flex-direction: column;
  align-items: center;
  border: none;
  background: none;
  cursor: pointer;
  padding: 4px 2px;
  border-radius: 4px;
}

.bar-group.selected {
  background: rgba(128, 128, 128, 0.12);
}

.bar-group:focus-visible {
  outline: 2px solid v-bind('themeVars.primaryColor');
  outline-offset: 1px;
}

.bar-stack {
  width: 20px;
  height: 150px;
  display: flex;
  flex-direction: column-reverse;
  justify-content: flex-start;
  border-radius: 3px;
  overflow: hidden;
}

.bar {
  width: 100%;
}

.bar-label {
  margin-top: 4px;
  font-size: 11px;
  color: v-bind('themeVars.textColor2');
}

.bar-value {
  font-size: 10px;
  font-weight: 600;
  color: v-bind('themeVars.textColor1');
}

.bar-rate {
  font-size: 10px;
  color: v-bind('themeVars.textColor3');
}

.trend-detail {
  margin-top: 12px;
}

.failure-list {
  --n-font-size: 13px;
}

.failure-item {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  width: 100%;
}

.failure-main {
  min-width: 0;
}

.failure-title {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.failure-message {
  margin: 4px 0;
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 640px;
}

.failure-time {
  margin: 0;
  font-size: 12px;
}

.muted {
  color: v-bind('themeVars.textColor3');
}
</style>
