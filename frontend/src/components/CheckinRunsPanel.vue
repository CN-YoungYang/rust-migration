<template>
  <div class="checkin-runs-panel">
    <div class="header">
      <div>
        <h2>签到记录</h2>
        <p class="panel-subtitle">
          当前加载 {{ runSummary.total }} 条，成功 {{ runSummary.succeeded }} 条，失败 {{ runSummary.failed }} 条
        </p>
      </div>
      <div class="header-actions">
        <select v-if="isAdmin" v-model="filterUserId" class="user-filter" aria-label="按用户筛选签到记录">
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
      <select v-model="filterTriggeredBy" class="filter-select" aria-label="按触发方式筛选">
        <option value="">全部触发方式</option>
        <option value="manual">手动</option>
        <option value="manual_batch">批量手动</option>
        <option value="scheduled">定时</option>
      </select>
      <div class="date-range">
        <input
          v-model="filterStartDate"
          type="datetime-local"
          class="filter-input"
          aria-label="开始时间"
          placeholder="开始时间"
        />
        <span class="date-separator">至</span>
        <input
          v-model="filterEndDate"
          type="datetime-local"
          class="filter-input"
          aria-label="结束时间"
          placeholder="结束时间"
        />
      </div>
      <select v-model="filterAccountId" class="filter-select" aria-label="按账户筛选">
        <option value="">全部账户</option>
        <optgroup v-for="group in groupedAccounts" :key="group.key" :label="group.label">
          <option v-for="account in group.items" :key="account.id" :value="account.id">
            {{ account.name }}
          </option>
        </optgroup>
      </select>
      <button v-if="hasActiveFilter" class="clear-filter" @click="clearFilters">清除筛选</button>
      <span class="filter-count">{{ runs.length }} 条记录</span>
    </div>

    <div class="summary-grid">
      <div class="summary-card">
        <strong>{{ runSummary.succeeded }}</strong>
        <span>成功或已签</span>
      </div>
      <div class="summary-card danger">
        <strong>{{ runSummary.failed }}</strong>
        <span>失败</span>
      </div>
      <div class="summary-card">
        <strong>{{ runSummary.pending }}</strong>
        <span>进行中</span>
      </div>
      <div class="summary-card">
        <strong>{{ runSummary.avgDuration }}ms</strong>
        <span>平均耗时</span>
      </div>
    </div>

    <div v-if="lastBatchResult" class="batch-result" role="status" aria-live="polite">
      <div class="batch-result-header">
        <div>
          <strong>批量重试结果</strong>
          <p class="muted">
            共 {{ lastBatchResult.total }} 个，成功 {{ lastBatchResult.succeeded }} 个，跳过 {{ lastBatchResult.skipped }} 个，失败 {{ lastBatchResult.failed }} 个
          </p>
        </div>
        <button class="ghost" @click="lastBatchResult = null">关闭</button>
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
              <span class="badge" :class="run.status.toLowerCase()">{{ statusText(run.status) }}</span>
              <span v-if="accountSite(run.accountId)" class="site-tag">{{ accountSite(run.accountId) }}</span>
            </div>
            <div class="run-meta">
              <span>触发方式: {{ triggerText(run.triggeredBy) }}</span>
              <span>时间: {{ formatTime(run.createdAt) }}</span>
              <span v-if="run.durationMs">耗时: {{ run.durationMs }}ms</span>
              <span v-if="accountOwner(run.accountId)">归属: {{ accountOwner(run.accountId) }}</span>
              <span v-if="run.message">消息: {{ run.message }}</span>
            </div>
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
            <button @click="copyRunSummary(run)">复制摘要</button>
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

const actionBusy = computed(() => executing.value || retryingBatch.value || cleaning.value)

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
      params.append('startDate', new Date(filterStartDate.value).toISOString())
    }
    if (filterEndDate.value) {
      params.append('endDate', new Date(filterEndDate.value).toISOString())
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

const triggerText = (trigger: string) => {
  const map: Record<string, string> = {
    manual: '手动',
    manual_batch: '批量手动',
    scheduled: '定时'
  }
  return map[trigger] || trigger
}

const formatTime = (time: string) => new Date(time).toLocaleString('zh-CN')

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

<style scoped>
/* design-system: design.md · Workbench panel · CheckinRunsPanel */
.checkin-runs-panel { max-width: 1200px; margin: 0 auto; padding: clamp(var(--space-sm), 2.5vw, var(--space-lg)) 0 var(--space-xl); }
.header { display: flex; justify-content: space-between; align-items: center; margin-bottom: var(--space-lg); gap: var(--space-sm); flex-wrap: wrap; }
.header-actions { display: flex; gap: var(--space-xs); align-items: center; flex-wrap: wrap; }
.user-filter { background: var(--bg-well); color: var(--color-ink); border: var(--rule-thin) solid var(--border-input); border-radius: var(--radius-input); padding: var(--space-2xs) var(--space-xs); font-size: var(--text-xs); }
h2 { color: var(--color-ink); }
.panel-subtitle { color: var(--text-muted); font-size: var(--text-meta); margin-top: var(--space-3xs); }
select, input { background: var(--bg-well); color: var(--color-ink); border: var(--rule-thin) solid var(--border-input); border-radius: var(--radius-input); padding: var(--space-2xs); }
.keep-input { width: 90px; }
.cleanup-controls { display: grid; gap: var(--space-3xs); padding: var(--space-2xs) var(--space-xs); background: var(--bg-card); border: var(--rule-thin) solid var(--border); border-radius: var(--radius-input); }
.cleanup-row { display: flex; gap: var(--space-2xs); align-items: center; }
.cleanup-scope { color: var(--text-muted); font-size: var(--text-xs); }
.cleanup-reset-option { display: flex; align-items: center; gap: var(--space-2xs); color: var(--text-faint); font-size: var(--text-xs); cursor: pointer; }
.cleanup-reset-option input { width: auto; margin: 0; padding: 0; accent-color: var(--accent); }
.filter-bar { display: flex; gap: var(--space-xs); align-items: center; flex-wrap: wrap; padding: var(--space-sm); background: var(--bg-card); border: var(--rule-thin) solid var(--border); border-radius: var(--radius-card); margin-bottom: var(--space-md); }
.filter-select { background: var(--bg-well); border: var(--rule-thin) solid var(--border-input); border-radius: var(--radius-input); color: var(--color-ink); padding: var(--space-2xs) var(--space-xs); font-size: var(--text-xs); }
.filter-input { background: var(--bg-well); border: var(--rule-thin) solid var(--border-input); border-radius: var(--radius-input); color: var(--color-ink); padding: var(--space-2xs) var(--space-xs); font-size: var(--text-xs); min-width: 180px; }
.date-range { display: flex; align-items: center; gap: var(--space-2xs); }
.date-separator { color: var(--color-muted); }
.status-filter { display: flex; gap: var(--space-2xs); }
.status-btn { background: var(--bg-elevated); border: var(--rule-thin) solid var(--border-strong); border-radius: var(--radius-pill); padding: var(--space-2xs) var(--space-xs); font-size: var(--text-meta); cursor: pointer; transition: background-color var(--dur-short) var(--ease-out), border-color var(--dur-short) var(--ease-out), color var(--dur-short) var(--ease-out); color: var(--color-ink); }
.status-btn:hover { background: var(--color-paper-2); }
.status-btn.active { background: var(--accent); border-color: var(--accent-border); color: var(--color-accent-ink); }
.status-btn .count { background: var(--color-paper-3); border-radius: var(--radius-pill); padding: var(--space-3xs) var(--space-2xs); margin-left: var(--space-3xs); font-size: var(--text-xs); }
.status-btn.active .count { color: var(--color-ink); }
.clear-filter { background: var(--color-paper-3); border: none; border-radius: var(--radius-input); padding: var(--space-2xs) var(--space-xs); color: var(--color-ink); font-size: var(--text-meta); cursor: pointer; }
.clear-filter:hover { background: var(--color-paper-2); }
.filter-count { color: var(--color-muted); font-size: var(--text-xs); margin-left: auto; }
.summary-grid { display: grid; grid-template-columns: repeat(4, minmax(140px, 1fr)); gap: var(--space-xs); margin-bottom: var(--space-md); }
.summary-card { background: var(--bg-card); border: var(--rule-thin) solid var(--border); border-radius: var(--radius-card); padding: var(--space-sm); display: grid; gap: var(--space-3xs); }
.summary-card strong { color: var(--text-strong); font-size: var(--text-summary); }
.summary-card span { color: var(--text-muted); font-size: var(--text-xs); }
.summary-card.danger strong { color: var(--color-danger); }
.batch-result { background: var(--bg-card); border: var(--rule-thin) solid var(--border); border-radius: var(--radius-card); padding: var(--space-sm); margin-bottom: var(--space-md); }
.batch-result-header { display: flex; align-items: center; justify-content: space-between; gap: var(--space-sm); }
.batch-items { display: grid; gap: var(--space-2xs); margin-top: var(--space-sm); max-height: 260px; overflow: auto; }
.batch-item { display: grid; grid-template-columns: minmax(160px, 1fr) auto minmax(160px, 2fr); align-items: center; gap: var(--space-xs); padding: var(--space-2xs) var(--space-xs); background: var(--bg-well); border: var(--rule-thin) solid var(--border); border-radius: var(--radius-input); }
.batch-name,
.batch-message { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.batch-name { color: var(--color-ink); font-weight: 600; }
.batch-message { color: var(--text-muted); }
.runs-list { display: grid; gap: var(--space-md); }
.run-group { display: grid; gap: var(--space-xs); }
.group-header { display: flex; align-items: center; gap: var(--space-2xs); padding-bottom: var(--space-3xs); border-bottom: var(--rule-thin) solid var(--border); }
.group-header strong { color: var(--color-ink); font-size: var(--text-md); }
.group-header .muted { font-size: var(--text-meta); }
.self-tag { background: var(--accent); border-radius: var(--radius-pill); padding: var(--space-3xs) var(--space-2xs); margin-left: var(--space-2xs); font-size: var(--text-xs); color: var(--color-accent-ink); font-weight: normal; }
.run-card { background: var(--bg-card); padding: var(--space-sm); border-radius: var(--radius-card); border: var(--rule-thin) solid var(--border); transition: background-color var(--dur-short) var(--ease-out), border-color var(--dur-short) var(--ease-out); display: flex; justify-content: space-between; gap: var(--space-sm); }
.run-card:hover { background: var(--color-paper-2); border-color: var(--border-strong); }
.run-card.success .badge { background: var(--success-soft); }
.run-card.failed .badge { background: var(--danger-soft); }
.run-card.already_checked .badge { background: var(--accent-soft); }
.run-card.pending .badge { background: var(--warn-soft); }
.run-info .account-name { color: var(--color-ink); font-size: var(--text-lg); font-weight: bold; }
.run-info { display: flex; flex-direction: column; gap: var(--space-2xs); }
.run-title { display: flex; align-items: center; gap: var(--space-2xs); flex-wrap: wrap; }
.run-meta { display: flex; flex-direction: column; gap: var(--space-3xs); color: var(--text-muted); font-size: var(--text-meta); }
.run-actions { display: flex; gap: var(--space-2xs); align-items: flex-start; flex-wrap: wrap; justify-content: flex-end; min-width: 160px; }
.badge { padding: var(--space-3xs) var(--space-2xs); border-radius: var(--radius-pill); font-size: var(--text-xs); display: inline-block; width: fit-content; background: var(--color-paper-3); color: var(--color-ink-2); }
.badge.success { background: var(--success-soft); color: var(--color-success); }
.badge.failed { background: var(--danger-soft); color: var(--color-danger); }
.badge.already_checked { background: var(--accent-soft); color: var(--color-accent-hover); }
.badge.pending { background: var(--color-warning-soft); color: var(--color-warning); }
.badge.neutral { background: var(--color-paper-3); }
.site-tag { background: var(--color-paper-3); color: var(--color-muted); border: var(--rule-thin) solid var(--border-strong); border-radius: var(--radius-pill); padding: var(--space-3xs) var(--space-2xs); font-size: var(--text-xs); }
button { color: var(--color-ink); border: none; padding: var(--space-2xs) var(--space-sm); border-radius: var(--radius-input); cursor: pointer; background: var(--color-paper-3); }
button:hover:not(:disabled) { background: var(--color-paper-2); }
button:disabled { background: var(--color-paper-3); cursor: not-allowed; opacity: 0.6; }
.btn-execute { background: var(--accent); color: var(--color-accent-ink); }
.btn-execute:hover:not(:disabled) { background: var(--accent-hover); }
.btn-retry { background: var(--color-accent-soft); color: var(--color-accent-hover); }
.btn-retry:hover:not(:disabled) { background: var(--color-accent-soft); }
.btn-cleanup { background: var(--color-danger-soft); color: var(--color-danger); }
.btn-cleanup:hover:not(:disabled) { background: var(--color-danger-soft); }
button.ghost { background: transparent; border: var(--rule-thin) solid var(--border-strong); color: var(--text-faint); }
button.ghost:hover:not(:disabled) { background: var(--bg-elevated); }
.empty { text-align: center; color: var(--text-muted); padding: var(--space-xl); background: var(--bg-card); border: var(--rule-thin) solid var(--border); border-radius: var(--radius-card); }
.load-more { text-align: center; padding: var(--space-sm); }
.load-more button { background: var(--color-paper-3); color: var(--color-ink-2); border: var(--rule-thin) solid var(--color-rule-strong); padding: var(--space-2xs) var(--space-md); border-radius: var(--radius-input); cursor: pointer; }
.load-more button:hover { background: var(--color-paper-2); color: var(--color-ink); }

@media (max-width: 47.99rem) {
  .checkin-runs-panel { padding: var(--space-sm); }
  .header-actions,
  .status-filter,
  .date-range { width: 100%; }
  .cleanup-row { width: 100%; }
  .cleanup-row .keep-input,
  .cleanup-row .btn-cleanup { flex: 1; width: auto; }
  .header-actions > *,
  .filter-select,
  .filter-input,
  .date-range input { width: 100%; }
  .status-filter { flex-wrap: wrap; }
  .filter-bar { flex-direction: column; align-items: stretch; }
  .filter-count { margin-left: 0; width: 100%; }
  .summary-grid { grid-template-columns: 1fr 1fr; }
  .run-card { display: grid; }
  .run-actions { justify-content: flex-start; min-width: 0; }
  .run-actions button { flex: 1; }
  .batch-item { grid-template-columns: 1fr; align-items: start; }
}
</style>
