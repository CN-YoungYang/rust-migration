<template>
  <div class="checkin-runs-panel">
    <div class="panel-header">
      <div>
        <h2>签到记录</h2>
        <p class="panel-subtitle">
          当前加载 {{ runs.length }} 条记录
        </p>
      </div>
      <div class="header-actions">
        <select v-if="isAdmin" v-model="filterUserId" aria-label="按用户筛选签到记录">
          <option value="">全部用户</option>
          <option v-if="usersLoading" disabled>加载中...</option>
          <option v-for="u in allUsers" :key="u.id" :value="u.id">{{ u.username }}</option>
        </select>
        <select v-model="selectedAccountId" aria-label="选择要执行签到的账户">
          <option value="">选择账户</option>
          <optgroup v-for="group in groupedAccounts" :key="group.key" :label="group.label">
            <option v-for="account in group.items" :key="account.id" :value="account.id">
              {{ account.name }} ({{ account.siteType }})
            </option>
          </optgroup>
        </select>
        <button @click="executeCheckin" class="btn-execute" :disabled="!selectedAccountId || executing" :data-state="executing ? 'loading' : undefined">
          {{ executing ? '执行中...' : '执行签到' }}
        </button>
        <button @click="retryFailedRuns" class="btn-retry" :disabled="failedAccountIds.length === 0 || actionBusy" :data-state="retryingBatch ? 'loading' : undefined">
          {{ retryingBatch ? '重试中...' : `重试失败账户 ${failedAccountIds.length}` }}
        </button>
        <div class="cleanup-controls">
          <span class="cleanup-scope">清理范围：{{ cleanupScope }}</span>
          <div class="cleanup-row">
            <input v-model.number="keepLatest" type="number" min="0" max="10000" class="keep-input" aria-label="清理后保留的最新记录数" title="保留最新记录数（0=清除全部）" />
            <button @click="cleanupRuns" class="btn-cleanup" :disabled="cleaning" :data-state="cleaning ? 'loading' : undefined">
              {{ cleaning ? '清理中...' : '清理历史' }}
            </button>
          </div>
          <label v-if="keepLatest === 0" class="cleanup-reset-option">
            <input v-model="resetState" type="checkbox" />
            同时重置最近签到状态和失败计数（保留余额）
          </label>
        </div>
      </div>
    </div>

    <!-- 筛选栏 -->
    <div class="filter-bar">
      <div class="status-filter" role="group" aria-label="按签到状态筛选">
        <button
          v-for="status in statusOptions"
          :key="status.value"
          :class="['status-btn', { active: filterStatus === status.value }]"
          :aria-pressed="filterStatus === status.value"
          @click="filterStatus = status.value"
        >
          {{ status.label }}
          <span v-if="statusCounts[status.value]" class="count">
            {{ statusCounts[status.value] }}
          </span>
        </button>
      </div>
      <select v-model="filterTriggeredBy" aria-label="按触发方式筛选">
        <option value="">全部触发方式</option>
        <option value="manual">手动</option>
        <option value="manual_batch">批量手动</option>
        <option value="scheduled">定时</option>
      </select>
      <div class="date-range">
        <input
          v-model="filterStartDate"
          type="date"
          aria-label="开始日期"
          placeholder="开始日期"
        />
        <span class="date-separator">至</span>
        <input
          v-model="filterEndDate"
          type="date"
          aria-label="结束日期"
          placeholder="结束日期"
        />
      </div>
      <select v-model="filterAccountId" aria-label="按账户筛选">
        <option value="">全部账户</option>
        <optgroup v-for="group in groupedAccounts" :key="group.key" :label="group.label">
          <option v-for="account in group.items" :key="account.id" :value="account.id">
            {{ account.name }}
          </option>
        </optgroup>
      </select>
      <button v-if="hasActiveFilter" @click="clearFilters">清除筛选</button>
      <span class="filter-count">{{ runs.length }} 条记录</span>
    </div>

    <dl class="summary-strip" aria-label="签到记录概览">
      <div>
        <dt>成功或已签</dt>
        <dd>{{ runSummary.succeeded }}</dd>
      </div>
      <div class="danger">
        <dt>失败</dt>
        <dd>{{ runSummary.failed }}</dd>
      </div>
      <div>
        <dt>进行中</dt>
        <dd>{{ runSummary.pending }}</dd>
      </div>
      <div>
        <dt>平均耗时</dt>
        <dd>{{ runSummary.avgDuration }}<small>ms</small></dd>
      </div>
    </dl>

    <div v-if="lastBatchResult" class="batch-result" role="status" aria-live="polite">
      <div class="batch-result-header">
        <div>
          <strong>批量重试结果</strong>
          <p class="muted">
            共 {{ lastBatchResult.total }} 个，成功 {{ lastBatchResult.succeeded }} 个，跳过 {{ lastBatchResult.skipped }} 个，失败 {{ lastBatchResult.failed }} 个
          </p>
        </div>
        <button @click="lastBatchResult = null">关闭</button>
      </div>
      <div class="batch-items">
        <div v-for="item in lastBatchResult.items" :key="item.accountId" class="batch-item">
          <span class="batch-name">{{ item.accountName }}</span>
          <span class="badge" :class="batchStatusClass(item.status)">{{ batchStatusText(item.status) }}</span>
          <span v-if="item.message" class="batch-message" :title="item.message">{{ item.message }}</span>
        </div>
      </div>
    </div>

    <div class="runs-list" :aria-busy="runsLoading">
      <section v-for="group in groupedRuns" :key="group.key" class="run-group">
        <div class="group-header">
          <strong>{{ group.label }}<span v-if="group.isSelf" class="self-tag">我</span></strong>
          <span class="muted">{{ group.items.length }} 条记录</span>
        </div>
        <div v-for="run in group.items" :key="run.id" class="run-card" :class="run.status.toLowerCase()">
          <div class="run-info">
            <div class="run-title">
              <span class="account-name">{{ accountName(run.accountId) }}</span>
              <span class="site-tag" v-if="accountSite(run.accountId)">{{ accountSite(run.accountId) }}</span>
              <span class="status-pill" :class="statusClass(run.status)">{{ statusText(run.status) }}</span>
            </div>
            <p class="run-meta">
              <span>{{ triggerText(run.triggeredBy) }}</span>
              <span>{{ formatTimeShort(run.createdAt) }}</span>
              <span v-if="run.durationMs">耗时 {{ run.durationMs }}ms</span>
              <span v-if="accountOwner(run.accountId)">归属 {{ accountOwner(run.accountId) }}</span>
            </p>
            <p v-if="run.message" class="run-message" :title="run.message">{{ run.message }}</p>
          </div>
          <div class="run-actions">
            <button
              v-if="run.status === 'failed'"
              class="btn-retry"
              :disabled="actionBusy"
              @click="executeAccountCheckin(run.accountId)"
            >
              {{ executingAccountId === run.accountId ? '重试中...' : '重试' }}
            </button>
            <button @click="copyRunSummary(run)" :disabled="actionBusy">复制摘要</button>
            <button
              class="btn-delete"
              :disabled="actionBusy"
              :data-state="deletingRunId === run.id ? 'loading' : undefined"
              @click="deleteRun(run)"
            >
              {{ deletingRunId === run.id ? '删除中...' : '删除' }}
            </button>
          </div>
        </div>
      </section>
      <div v-if="runs.length === 0 && !runsLoading" class="empty" role="status">暂无签到记录</div>
      <div v-if="runsLoading" class="empty" role="status" aria-live="polite">加载中...</div>
      <div v-if="hasMore && runs.length > 0 && !runsLoading" class="load-more">
        <button @click="loadMoreRuns">加载更多</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { apiUrl, request, responseData } from '../utils/api'
import { confirmAction, showToast } from '../utils/toast'
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

interface RunGroup {
  key: string
  label: string
  isSelf: boolean
  items: CheckinRun[]
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

const statusOptions = [
  { value: '', label: '全部' },
  { value: 'success', label: '成功' },
  { value: 'failed', label: '失败' },
  { value: 'already_checked', label: '已签' },
  { value: 'pending', label: '进行中' }
]

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
  return !!(filterStatus.value || filterTriggeredBy.value || filterStartDate.value || filterEndDate.value || filterAccountId.value)
})

function clearFilters() {
  filterStatus.value = ''
  filterTriggeredBy.value = ''
  filterStartDate.value = ''
  filterEndDate.value = ''
  filterAccountId.value = ''
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

// 通过账户反查归属用户，把签到记录按用户分组；当前用户分组置顶。
const groupedRuns = computed<RunGroup[]>(() => {
  const groups = new Map<string, RunGroup>()
  for (const run of runs.value) {
    const account = accounts.value.find((a) => a.id === run.accountId)
    const key = account?.ownerId || 'unknown'
    if (!groups.has(key)) {
      const label = account?.ownerName
        || (account?.ownerId ? `用户 ${account.ownerId.slice(0, 8)}` : '已删除账户')
      groups.set(key, {
        key,
        label,
        isSelf: !!props.currentUser && !!account?.ownerId && account.ownerId === props.currentUser.id,
        items: [],
      })
    }
    groups.get(key)!.items.push(run)
  }
  return Array.from(groups.values()).sort((a, b) => {
    if (a.isSelf !== b.isSelf) return a.isSelf ? -1 : 1
    return a.label.localeCompare(b.label, 'zh-Hans')
  })
})

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
      showToast(error instanceof Error ? error.message : '加载账户失败', 'error')
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
      showToast(error instanceof Error ? error.message : '加载签到记录失败', 'error')
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

async function confirmDailyLimit(accountId: string): Promise<boolean> {
  const account = accounts.value.find((a) => a.id === accountId)
  if (account && (account.todayRuns ?? 0) >= maxAttemptsPerDay.value) {
    return confirmAction(
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
    showToast(error instanceof Error ? error.message : '执行签到失败', 'error')
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
    if (result.failed > 0) showToast(`重试后仍有 ${result.failed} 个账户失败`, 'error')
    await Promise.all([fetchRuns(), fetchAccounts()])
  } catch (error) {
    showToast(error instanceof Error ? error.message : '重试失败账户失败', 'error')
  } finally {
    retryingBatch.value = false
  }
}

const cleanupRuns = async () => {
  if (cleaning.value) return
  if (!Number.isInteger(keepLatest.value) || keepLatest.value < 0 || keepLatest.value > 10000) {
    showToast('保留数量必须是 0~10000 的整数', 'error')
    return
  }

  const resetDescription = keepLatest.value === 0 && resetState.value
    ? '，并重置最近签到状态和失败计数（余额保留）'
    : ''
  const msg = keepLatest.value === 0
    ? `确定清空${cleanupTarget.value}签到历史${resetDescription}吗？此操作不可撤销！`
    : `确定清理${cleanupTarget.value}签到历史，并保留最新 ${keepLatest.value} 条吗？`
  if (!(await confirmAction(msg))) return
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
    showToast(`已删除 ${result.deletedCount} 条签到历史${resetSummary}`, 'success')
    await Promise.all([fetchRuns(), fetchAccounts()])
  } catch (error) {
    showToast(error instanceof Error ? error.message : '清理签到历史失败', 'error')
  } finally {
    cleaning.value = false
  }
}
const deleteRun = async (run: CheckinRun) => {
  if (!run?.id || deletingRunId.value) return
  const name = accountName(run.accountId)
  if (!(await confirmAction(`确定删除账户「${name}」的这条签到记录吗？此操作不可撤销。`))) return

  deletingRunId.value = run.id
  try {
    await request(apiUrl(`/checkin-runs/${encodeURIComponent(run.id)}`), {
      method: 'DELETE',
    })
    showToast('已删除该签到记录', 'success')
    runs.value = runs.value.filter((item) => item.id !== run.id)
    await fetchRuns()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '删除签到记录失败', 'error')
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

const statusText = (status: string) => {
  const normalized = status.toLowerCase()
  const map: Record<string, string> = {
    success: '成功',
    failed: '失败',
    already_checked: '今日已签',
    pending: '进行中'
  }
  return map[normalized] || status
}

// 徽标类名映射：已签用 workbench 的 .already 配色
const statusClass = (status: string): string => {
  const normalized = status.toLowerCase()
  return normalized === 'already_checked' ? 'already' : normalized
}

const triggerText = (trigger: string) => {
  const map: Record<string, string> = {
    manual: '手动',
    manual_batch: '批量手动',
    scheduled: '定时'
  }
  return map[trigger] || trigger
}

const formatTime = (time: string) => new Date(time).toLocaleString('zh-CN')

// 记录行内的紧凑时间：MM-DD HH:mm，降低整段信息的视觉重量
const formatTimeShort = (time: string): string => {
  const date = new Date(time)
  if (Number.isNaN(date.getTime())) return '—'
  return date.toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

// 把日期选择器的 `YYYY-MM-DD` 转成浏览器本地时区的日界 RFC3339 时刻。
// 记录在界面上按浏览器本地时间显示，筛选也必须用浏览器本地日界，否则
// 服务器时区与浏览器时区不一致时会筛错日期（回归修复：此前把裸日期字符串
// 交给后端按服务器时区解释）。后端对含 `T` 的时间戳原样透传，作为绝对时刻比较。
const dayBoundary = (date: string, atEnd: boolean): string => {
  const time = atEnd ? 'T23:59:59.999' : 'T00:00:00'
  return new Date(`${date}${time}`).toISOString()
}

const batchStatusText = (status: string) => {
  const map: Record<string, string> = {
    success: '成功',
    failed: '失败',
    skipped: '跳过',
    already_checked: '今日已签',
    pending: '进行中',
  }
  return map[status] || status
}

const batchStatusClass = (status: string) => {
  if (status === 'already_checked') return 'already_checked'
  if (status === 'skipped') return 'neutral'
  return status
}

const copyRunSummary = async (run: CheckinRun) => {
  const summary = [
    `账户: ${accountName(run.accountId)}`,
    `站点: ${accountSite(run.accountId) || '-'}`,
    `状态: ${statusText(run.status)}`,
    `触发: ${triggerText(run.triggeredBy)}`,
    `时间: ${formatTime(run.createdAt)}`,
    `耗时: ${run.durationMs ? `${run.durationMs}ms` : '-'}`,
    `消息: ${run.message || '-'}`,
  ].join('\n')

  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(summary)
    } else {
      const textarea = document.createElement('textarea')
      textarea.value = summary
      textarea.style.position = 'fixed'
      textarea.style.opacity = '0'
      document.body.appendChild(textarea)
      textarea.select()
      document.execCommand('copy')
      document.body.removeChild(textarea)
    }
    showToast('摘要已复制', 'success')
  } catch {
    showToast('复制失败，请手动选择消息内容', 'error')
  }
}

onMounted(async () => {
  try {
    await Promise.all([fetchAccounts(), fetchRuns(), fetchUsers(), fetchSettings()])
  } catch (error) {
    showToast(error instanceof Error ? error.message : '加载失败', 'error')
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

<style scoped src="./CheckinRunsPanel.css"></style>
