<template>
  <div class="panel">
    <div class="panel-header">
      <div>
        <h2 class="panel-title">签到记录</h2>
        <n-text depth="3" class="panel-subtitle">当前加载 {{ runs.length }} 条记录</n-text>
      </div>
      <n-space align="center" :size="8" wrap>
        <n-select
          v-if="isAdmin"
          v-model:value="filterUserId"
          :options="userFilterOptions"
          placeholder="全部用户"
          :loading="usersLoading"
          size="small"
          style="width: 140px"
        />
        <n-select
          v-model:value="selectedAccountId"
          :options="accountSelectOptions"
          placeholder="选择账户"
          clearable
          size="small"
          style="width: 200px"
        />
        <n-button
          size="small"
          type="primary"
          :loading="executing"
          :disabled="!selectedAccountId || executing"
          @click="executeCheckin"
        >
          {{ executing ? '执行中…' : '执行签到' }}
        </n-button>
        <n-button size="small" :disabled="failedAccountIds.length === 0 || actionBusy" :loading="retryingBatch" @click="retryFailedRuns">
          {{ retryingBatch ? '重试中…' : `重试失败账户 ${failedAccountIds.length}` }}
        </n-button>
        <div class="cleanup-controls">
          <span class="muted cleanup-scope">清理范围：{{ cleanupScope }}</span>
          <n-input-number
            v-model:value="keepLatest"
            :min="0"
            :max="10000"
            size="small"
            class="keep-input"
            :title="'保留最新记录数（0=清除全部）'"
            aria-label="清理后保留的最新记录数"
          />
          <n-button size="small" :loading="cleaning" :disabled="actionBusy" @click="cleanupRuns">
            {{ cleaning ? '清理中…' : '清理历史' }}
          </n-button>
          <n-checkbox v-if="keepLatest === 0" v-model:checked="resetState" size="small" class="cleanup-reset-option">
            同时重置最近签到状态和失败计数（保留余额）
          </n-checkbox>
        </div>
      </n-space>
    </div>

    <!-- 筛选栏 -->
    <n-space class="filter-bar" align="center" :size="8" wrap>
      <n-radio-group v-model:value="filterStatus" size="small">
        <n-radio-button v-for="s in statusOptions" :key="s.value" :value="s.value">
          {{ s.label }}
          <span v-if="statusCounts[s.value]" class="count-badge">{{ statusCounts[s.value] }}</span>
        </n-radio-button>
      </n-radio-group>
      <n-select
        v-model:value="filterTriggeredBy"
        :options="triggerOptions"
        size="small"
        style="width: 130px"
      />
      <n-date-picker
        v-model:value="dateRange"
        type="daterange"
        clearable
        size="small"
        style="width: 260px"
      />
      <n-select
        v-model:value="filterAccountId"
        :options="accountFilterOptions"
        placeholder="全部账户"
        clearable
        size="small"
        style="width: 180px"
      />
      <n-button v-if="hasActiveFilter" size="small" @click="clearFilters">清除筛选</n-button>
      <n-text depth="3" class="filter-count">{{ runs.length }} 条记录</n-text>
    </n-space>

    <!-- 概览 -->
    <n-grid class="summary-grid" :cols="4" :x-gap="12" :y-gap="12" responsive="screen" item-responsive>
      <n-grid-item>
        <n-statistic label="成功或已签" :value="runSummary.succeeded" />
      </n-grid-item>
      <n-grid-item>
        <n-statistic
          label="失败"
          :value="runSummary.failed"
          :value-style="runSummary.failed > 0 ? { color: themeVars.errorColor } : undefined"
        />
      </n-grid-item>
      <n-grid-item>
        <n-statistic label="进行中" :value="runSummary.pending" />
      </n-grid-item>
      <n-grid-item>
        <n-statistic label="平均耗时">
          {{ runSummary.avgDuration }}<small class="unit">ms</small>
        </n-statistic>
      </n-grid-item>
    </n-grid>

    <!-- 批量重试结果 -->
    <n-card v-if="lastBatchResult" size="small" class="batch-result" role="status" aria-live="polite">
      <template #header>
        <div class="batch-result-header">
          <strong>批量重试结果</strong>
          <span class="muted">
            共 {{ lastBatchResult.total }} 个，成功 {{ lastBatchResult.succeeded }} 个，跳过 {{ lastBatchResult.skipped }} 个，失败 {{ lastBatchResult.failed }} 个
          </span>
        </div>
      </template>
      <template #header-extra>
        <n-button size="tiny" text @click="lastBatchResult = null">关闭</n-button>
      </template>
      <n-list>
        <n-list-item v-for="item in lastBatchResult.items" :key="item.accountId">
          <div class="batch-item">
            <span class="batch-name">{{ item.accountName }}</span>
            <n-tag size="small" :bordered="false" :type="checkinStatusTagType(item.status)">
              {{ checkinStatusText(item.status) }}
            </n-tag>
            <span v-if="item.message" class="batch-message muted" :title="item.message">{{ item.message }}</span>
          </div>
        </n-list-item>
      </n-list>
    </n-card>

    <!-- 记录表 -->
    <n-data-table
      :columns="runColumns"
      :data="runs"
      :loading="runsLoading"
      :row-key="(row: CheckinRun) => row.id"
      :scroll-x="900"
      class="runs-table"
    >
      <template #empty>暂无签到记录</template>
    </n-data-table>
    <div v-if="hasMore && runs.length > 0 && !runsLoading" class="load-more">
      <n-button size="small" @click="loadMoreRuns">加载更多</n-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, h, onMounted, ref, watch } from 'vue'
import {
  NButton,
  NCard,
  NCheckbox,
  NDataTable,
  NDatePicker,
  NGrid,
  NGridItem,
  NInputNumber,
  NList,
  NListItem,
  NPopconfirm,
  NRadioButton,
  NRadioGroup,
  NSelect,
  NSpace,
  NStatistic,
  NTag,
  NText,
  useDialog,
  useMessage,
  useThemeVars,
  type DataTableColumns,
} from 'naive-ui'
import { apiUrl, request, responseData } from '../utils/api'
import { formatDateTime, formatDateTimeFull, formatDateInput } from '../utils/format'
import { checkinStatusText, checkinStatusTagType, triggerText } from '../utils/checkinStatus'
import { copyText } from '../utils/clipboard'
import type { CurrentUser, Account, AccountGroup } from '../types'
import { useUsers } from '../composables/useUsers'
import { buildCleanupRequest, cleanupScopeLabel, cleanupTargetText } from '../utils/cleanupRuns'

interface CheckinRun {
  id: string
  accountId: string
  status: string
  message?: string | null
  durationMs?: number | null
  triggeredBy: string
  rawResponse?: string | null
  createdAt: string
}

interface BatchResultItem {
  accountId: string
  accountName: string
  status: string
  message?: string | null
}

interface BatchCheckinResult {
  items: BatchResultItem[]
  total: number
  succeeded: number
  skipped: number
  failed: number
}
interface CleanupRunsResult {
  deletedCount: number
  keepLatest: number
  resetAccountCount: number
  deletedFailureCounterCount: number
  userId: string | null
}

const props = defineProps<{
  currentUser: CurrentUser | null
  isAdmin: boolean
}>()

const message = useMessage()
const dialog = useDialog()
const themeVars = useThemeVars()

const { allUsers, usersLoading, loadUsers: fetchUsers } = useUsers(() => props.isAdmin)
const filterUserId = ref('')

const accounts = ref<Account[]>([])
const runs = ref<CheckinRun[]>([])
const selectedAccountId = ref('')
const keepLatest = ref(100)
const resetState = ref(true)
const runsLoading = ref(false)
const runsOffset = ref(0)
const hasMore = ref(true)
const executing = ref(false)
const executingAccountId = ref('')
const retryingBatch = ref(false)
const cleaning = ref(false)
const deletingRunId = ref('')
const PAGE_SIZE = 100
let accountRequestSeq = 0
let runsRequestSeq = 0
const maxAttemptsPerDay = ref(3)
const cleanupScope = computed(() => {
  const selectedUsername = allUsers.value.find((user) => user.id === filterUserId.value)?.username || ''
  return cleanupScopeLabel(props.isAdmin, filterUserId.value, selectedUsername)
})
const cleanupTarget = computed(() => {
  const selectedUsername = allUsers.value.find((user) => user.id === filterUserId.value)?.username || ''
  return cleanupTargetText(props.isAdmin, filterUserId.value, selectedUsername)
})
const lastBatchResult = ref<BatchCheckinResult | null>(null)

// 筛选相关
const filterStatus = ref('')
const filterTriggeredBy = ref('')
const filterStartDate = ref('')
const filterEndDate = ref('')
const filterAccountId = ref('')
const dateRange = ref<[number, number] | null>(null)

const statusOptions = [
  { value: '', label: '全部' },
  { value: 'success', label: '成功' },
  { value: 'failed', label: '失败' },
  { value: 'already_checked', label: '已签' },
  { value: 'pending', label: '进行中' },
]

const triggerOptions = [
  { label: '全部触发方式', value: '' },
  { label: '手动', value: 'manual' },
  { label: '批量手动', value: 'manual_batch' },
  { label: '定时', value: 'scheduled' },
]

const userFilterOptions = computed(() => {
  const options = [{ label: '全部用户', value: '' }]
  for (const user of allUsers.value) {
    options.push({ label: user.username, value: user.id })
  }
  return options
})

const statusCounts = computed(() => {
  const counts: Record<string, number> = {}
  for (const run of runs.value) {
    counts[run.status] = (counts[run.status] || 0) + 1
  }
  return counts
})

const actionBusy = computed(() => executing.value || retryingBatch.value || cleaning.value || Boolean(deletingRunId.value))

const accountById = computed(() => {
  return new Map(accounts.value.map((account) => [account.id, account]))
})

const failedAccountIds = computed(() => {
  const ids = runs.value
    .filter((run) => run.status === 'failed')
    .map((run) => run.accountId)
  return [...new Set(ids)]
})

const runSummary = computed(() => {
  let succeeded = 0
  let failed = 0
  let pending = 0
  let durationTotal = 0
  let durationCount = 0

  for (const run of runs.value) {
    if (run.status === 'success' || run.status === 'already_checked') succeeded += 1
    if (run.status === 'failed') failed += 1
    if (run.status === 'pending') pending += 1
    if (typeof run.durationMs === 'number') {
      durationTotal += run.durationMs
      durationCount += 1
    }
  }

  return {
    total: runs.value.length,
    succeeded,
    failed,
    pending,
    avgDuration: durationCount > 0 ? Math.round(durationTotal / durationCount) : 0,
  }
})

const hasActiveFilter = computed(() => {
  return !!(filterUserId.value || filterStatus.value || filterTriggeredBy.value || filterStartDate.value || filterEndDate.value || filterAccountId.value)
})

function clearFilters() {
  filterUserId.value = ''
  filterStatus.value = ''
  filterTriggeredBy.value = ''
  filterStartDate.value = ''
  filterEndDate.value = ''
  filterAccountId.value = ''
  dateRange.value = null
}

// 按账户归属用户分组下拉框选项
const groupedAccounts = computed<AccountGroup[]>(() => {
  const groups = new Map<string, AccountGroup>()
  for (const account of accounts.value) {
    const key = account.ownerId || 'unknown'
    if (!groups.has(key)) {
      const label = account.ownerName || (account.ownerId ? `用户 ${account.ownerId.slice(0, 8)}` : '未知用户')
      groups.set(key, { key, label, items: [] })
    }
    groups.get(key)!.items.push(account)
  }
  return Array.from(groups.values()).sort((a, b) => {
    const aSelf = !!props.currentUser && a.key === props.currentUser.id
    const bSelf = !!props.currentUser && b.key === props.currentUser.id
    if (aSelf !== bSelf) return aSelf ? -1 : 1
    return a.label.localeCompare(b.label, 'zh-Hans')
  })
})

function accountGroupOptions(labelFn: (account: Account) => string) {
  return groupedAccounts.value.map((group) => ({
    type: 'group' as const,
    label: group.label,
    key: group.key,
    children: group.items.map((account) => ({
      label: labelFn(account),
      value: account.id,
    })),
  }))
}

const accountSelectOptions = computed(() => accountGroupOptions((account) => `${account.name} (${account.siteType})`))
const accountFilterOptions = computed(() => accountGroupOptions((account) => account.name))

const fetchAccounts = async () => {
  const seq = ++accountRequestSeq
  try {
    let url = apiUrl('/accounts')
    if (props.isAdmin && filterUserId.value) {
      url += `?userId=${encodeURIComponent(filterUserId.value)}`
    }
    const response = await request(url)
    const data = await responseData<Account[]>(response)
    if (seq !== accountRequestSeq) return
    accounts.value = data
    // 如果当前选中的账户不在新列表中，清除选择
    if (selectedAccountId.value && !accounts.value.find((a) => a.id === selectedAccountId.value)) {
      selectedAccountId.value = ''
    }
    if (!selectedAccountId.value && accounts.value.length > 0) {
      selectedAccountId.value = accounts.value[0].id
    }
  } catch (error) {
    if (seq === accountRequestSeq) {
      message.error(error instanceof Error ? error.message : '加载账户失败')
    }
  }
}

const fetchRuns = async (append = false) => {
  const seq = ++runsRequestSeq
  runsLoading.value = true
  try {
    const offset = append ? runsOffset.value : 0
    let url = apiUrl('/checkin-runs')
    const params = new URLSearchParams()

    params.append('limit', PAGE_SIZE.toString())
    params.append('offset', offset.toString())

    if (props.isAdmin && filterUserId.value) {
      params.append('userId', filterUserId.value)
    }
    if (filterStatus.value) {
      params.append('status', filterStatus.value)
    }
    if (filterTriggeredBy.value) {
      params.append('triggeredBy', filterTriggeredBy.value)
    }
    if (filterStartDate.value) {
      params.append('startDate', dayBoundary(filterStartDate.value, false))
    }
    if (filterEndDate.value) {
      params.append('endDate', dayBoundary(filterEndDate.value, true))
    }
    if (filterAccountId.value) {
      params.append('accountId', filterAccountId.value)
    }

    url += `?${params.toString()}`

    const response = await request(url)
    const data = await responseData<CheckinRun[]>(response)
    if (seq !== runsRequestSeq) return
    if (append) {
      runs.value.push(...data)
    } else {
      runs.value = data
      runsOffset.value = 0
    }
    runsOffset.value += data.length
    hasMore.value = data.length >= PAGE_SIZE
  } catch (error) {
    if (seq === runsRequestSeq) {
      message.error(error instanceof Error ? error.message : '加载签到记录失败')
    }
  } finally {
    if (seq === runsRequestSeq) runsLoading.value = false
  }
}

const loadMoreRuns = () => fetchRuns(true)

const fetchSettings = async () => {
  try {
    const res = await request(apiUrl('/settings'))
    const data = await responseData<{ maxAttemptsPerDay?: number }>(res)
    maxAttemptsPerDay.value = data.maxAttemptsPerDay ?? 3
  } catch {
    // 使用默认值
  }
}

function confirmWarning(content: string): Promise<boolean> {
  return new Promise((resolve) => {
    dialog.warning({
      title: '请确认',
      content,
      positiveText: '确认',
      negativeText: '取消',
      onPositiveClick: () => resolve(true),
      onNegativeClick: () => resolve(false),
      onClose: () => resolve(false),
    })
  })
}

async function confirmDailyLimit(accountId: string): Promise<boolean> {
  const account = accounts.value.find((a) => a.id === accountId)
  if (account && (account.todayRuns ?? 0) >= maxAttemptsPerDay.value) {
    return confirmWarning(
      `该账户今日已签到 ${account.todayRuns} 次，已达每日上限（${maxAttemptsPerDay.value} 次）。\n手动签到不受限制，确定继续吗？`
    )
  }
  return true
}

const executeCheckin = async () => {
  if (!selectedAccountId.value) return
  await executeAccountCheckin(selectedAccountId.value)
}

const executeAccountCheckin = async (accountId: string) => {
  if (!accountId || executing.value) return
  if (!(await confirmDailyLimit(accountId))) return

  executing.value = true
  executingAccountId.value = accountId
  try {
    await request(apiUrl('/checkin-runs'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ accountId })
    })
    await Promise.all([fetchRuns(), fetchAccounts()])
  } catch (error) {
    message.error(error instanceof Error ? error.message : '执行签到失败')
  } finally {
    executing.value = false
    executingAccountId.value = ''
  }
}

const retryFailedRuns = async () => {
  const accountIds = failedAccountIds.value
  if (accountIds.length === 0 || retryingBatch.value) return

  retryingBatch.value = true
  lastBatchResult.value = null
  try {
    const response = await request(apiUrl('/checkin-runs/batch'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ accountIds })
    })
    const result = await responseData<BatchCheckinResult>(response)
    lastBatchResult.value = result
    if (result.failed > 0) message.error(`重试后仍有 ${result.failed} 个账户失败`)
    await Promise.all([fetchRuns(), fetchAccounts()])
  } catch (error) {
    message.error(error instanceof Error ? error.message : '重试失败账户失败')
  } finally {
    retryingBatch.value = false
  }
}

const cleanupRuns = async () => {
  if (cleaning.value) return
  if (!Number.isInteger(keepLatest.value) || keepLatest.value < 0 || keepLatest.value > 10000) {
    message.error('保留数量必须是 0~10000 的整数')
    return
  }

  const resetDescription = keepLatest.value === 0 && resetState.value
    ? '，并重置最近签到状态和失败计数（余额保留）'
    : ''
  const msg = keepLatest.value === 0
    ? `确定清空${cleanupTarget.value}签到历史${resetDescription}吗？此操作不可撤销！`
    : `确定清理${cleanupTarget.value}签到历史，并保留最新 ${keepLatest.value} 条吗？`
  if (!(await confirmWarning(msg))) return
  cleaning.value = true
  try {
    const response = await request(apiUrl('/checkin-runs/cleanup'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(buildCleanupRequest(
        keepLatest.value,
        props.isAdmin,
        filterUserId.value,
        resetState.value,
      ))
    })
    const result = await responseData<CleanupRunsResult>(response)
    const resetSummary = result.resetAccountCount > 0 || result.deletedFailureCounterCount > 0
      ? `，重置 ${result.resetAccountCount} 个账户状态和 ${result.deletedFailureCounterCount} 个失败计数`
      : ''
    message.success(`已删除 ${result.deletedCount} 条签到历史${resetSummary}`)
    await Promise.all([fetchRuns(), fetchAccounts()])
  } catch (error) {
    message.error(error instanceof Error ? error.message : '清理签到历史失败')
  } finally {
    cleaning.value = false
  }
}

const deleteRun = async (run: CheckinRun) => {
  if (!run?.id || deletingRunId.value) return
  deletingRunId.value = run.id
  try {
    await request(apiUrl(`/checkin-runs/${encodeURIComponent(run.id)}`), {
      method: 'DELETE',
    })
    message.success('已删除该签到记录')
    runs.value = runs.value.filter((item) => item.id !== run.id)
    await fetchRuns()
  } catch (error) {
    message.error(error instanceof Error ? error.message : '删除签到记录失败')
  } finally {
    deletingRunId.value = ''
  }
}

const accountName = (accountId: string) => {
  return accountById.value.get(accountId)?.name || accountId
}

const accountSite = (accountId: string) => {
  return accountById.value.get(accountId)?.siteType || ''
}

const accountOwner = (accountId: string) => {
  return accountById.value.get(accountId)?.ownerName || ''
}

// 把日期选择器的 `YYYY-MM-DD` 转成浏览器本地时区的日界 RFC3339 时刻。
// 记录在界面上按浏览器本地时间显示，筛选也必须用浏览器本地日界，否则
// 服务器时区与浏览器时区不一致时会筛错日期（回归修复：此前把裸日期字符串
// 交给后端按服务器时区解释）。后端对含 `T` 的时间戳原样透传，作为绝对时刻比较。
const dayBoundary = (date: string, atEnd: boolean): string => {
  const time = atEnd ? 'T23:59:59.999' : 'T00:00:00'
  return new Date(`${date}${time}`).toISOString()
}

watch(dateRange, (range) => {
  if (!range) {
    filterStartDate.value = ''
    filterEndDate.value = ''
    return
  }
  const [start, end] = range
  filterStartDate.value = formatDateInput(new Date(start))
  filterEndDate.value = formatDateInput(new Date(end))
})

const copyRunSummary = async (run: CheckinRun) => {
  const summary = [
    `账户: ${accountName(run.accountId)}`,
    `站点: ${accountSite(run.accountId) || '-'}`,
    `状态: ${checkinStatusText(run.status)}`,
    `触发: ${triggerText(run.triggeredBy)}`,
    `时间: ${formatDateTimeFull(run.createdAt)}`,
    `耗时: ${run.durationMs ? `${run.durationMs}ms` : '-'}`,
    `消息: ${run.message || '-'}`,
  ].join('\n')

  try {
    await copyText(summary)
    message.success('摘要已复制')
  } catch {
    message.error('复制失败，请手动选择消息内容')
  }
}

const runColumns = computed<DataTableColumns<CheckinRun>>(() => {
  // 显式依赖，保证操作中表格重渲染
  const busy = actionBusy.value
  const executingId = executingAccountId.value
  const deletingId = deletingRunId.value
  void busy
  void executingId
  void deletingId
  void accounts.value

  return [
    {
      title: '时间',
      key: 'createdAt',
      width: 120,
      render: (row) =>
        h('span', { title: formatDateTimeFull(row.createdAt) }, formatDateTime(row.createdAt, '—')),
    },
    {
      title: '账户',
      key: 'accountId',
      render: (row) =>
        h('div', { class: 'account-name-cell' }, [
          h('span', { class: 'account-name' }, accountName(row.accountId)),
          accountSite(row.accountId)
            ? h(NTag, { size: 'small', bordered: false }, { default: () => accountSite(row.accountId) })
            : null,
        ]),
    },
    {
      title: '状态',
      key: 'status',
      width: 100,
      render: (row) =>
        h(
          NTag,
          { size: 'small', bordered: false, type: checkinStatusTagType(row.status) },
          { default: () => checkinStatusText(row.status) },
        ),
    },
    { title: '触发', key: 'triggeredBy', width: 90, render: (row) => triggerText(row.triggeredBy) },
    { title: '耗时', key: 'durationMs', width: 80, render: (row) => (row.durationMs ? `${row.durationMs}ms` : '—') },
    {
      title: '消息',
      key: 'message',
      ellipsis: { tooltip: true },
      render: (row) => row.message || '—',
    },
    ...(props.isAdmin
      ? [{ title: '归属', key: 'owner', width: 100, render: (row: CheckinRun) => accountOwner(row.accountId) || '—' }]
      : []),
    {
      title: '操作',
      key: 'actions',
      width: 210,
      render: (row) =>
        h(NSpace, { size: 4 }, {
          default: () => [
            row.status === 'failed'
              ? h(
                  NButton,
                  {
                    size: 'tiny',
                    secondary: true,
                    loading: executingId === row.accountId && executing.value,
                    disabled: busy,
                    onClick: () => executeAccountCheckin(row.accountId),
                  },
                  { default: () => (executingId === row.accountId ? '重试中…' : '重试') },
                )
              : null,
            h(
              NButton,
              {
                size: 'tiny',
                tertiary: true,
                disabled: busy,
                onClick: () => copyRunSummary(row),
              },
              { default: () => '复制摘要' },
            ),
            h(
              NPopconfirm,
              {
                onPositiveClick: () => deleteRun(row),
              },
              {
                trigger: () =>
                  h(
                    NButton,
                    {
                      size: 'tiny',
                      tertiary: true,
                      type: 'error',
                      loading: deletingId === row.id,
                      disabled: busy,
                    },
                    { default: () => (deletingId === row.id ? '删除中…' : '删除') },
                  ),
                default: () => `确定删除账户「${accountName(row.accountId)}」的这条签到记录吗？此操作不可撤销。`,
              },
            ),
          ],
        }),
    },
  ]
})

onMounted(async () => {
  try {
    await Promise.all([fetchAccounts(), fetchRuns(), fetchUsers(), fetchSettings()])
  } catch (error) {
    message.error(error instanceof Error ? error.message : '加载失败')
  }
})

watch(filterUserId, () => {
  selectedAccountId.value = ''
  filterAccountId.value = ''
  fetchAccounts()
  fetchRuns()
})

watch([filterStatus, filterTriggeredBy, filterStartDate, filterEndDate, filterAccountId], () => {
  fetchRuns()
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

.filter-bar {
  margin-bottom: 14px;
}

.count-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 18px;
  height: 18px;
  padding: 0 5px;
  margin-left: 4px;
  border-radius: 9px;
  font-size: 12px;
  background: color-mix(in srgb, v-bind('themeVars.textColor3') 16%, transparent);
}

.summary-grid {
  margin-bottom: 14px;
  font-variant-numeric: tabular-nums;
}

.unit {
  font-size: 12px;
  margin-left: 2px;
}

.batch-result {
  margin-bottom: 12px;
}

.batch-result-header {
  display: flex;
  align-items: baseline;
  gap: 10px;
}

.batch-item {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.batch-name {
  font-weight: 500;
  flex: none;
}

.batch-message {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cleanup-controls {
  display: flex;
  align-items: center;
  gap: 8px;
}

.cleanup-scope {
  font-size: 13px;
}

.keep-input {
  width: 110px;
}

.cleanup-reset-option {
  font-size: 13px;
}

.runs-table {
  margin-top: 4px;
  font-variant-numeric: tabular-nums;
}

.load-more {
  display: flex;
  justify-content: center;
  margin-top: 12px;
}

.account-name-cell {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.account-name {
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 200px;
}

.muted {
  color: v-bind('themeVars.textColor3');
}
</style>
