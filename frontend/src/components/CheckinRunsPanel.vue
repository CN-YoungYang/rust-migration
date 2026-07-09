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
        <select v-if="isAdmin" v-model="filterUserId" class="user-filter">
          <option value="">全部用户</option>
          <option v-if="usersLoading" disabled>加载中...</option>
          <option v-for="u in allUsers" :key="u.id" :value="u.id">{{ u.username }}</option>
        </select>
        <select v-model="selectedAccountId">
          <option value="">选择账户</option>
          <optgroup v-for="group in groupedAccounts" :key="group.key" :label="group.label">
            <option v-for="account in group.items" :key="account.id" :value="account.id">
              {{ account.name }} ({{ account.siteType }})
            </option>
          </optgroup>
        </select>
        <button @click="executeCheckin" class="btn-execute" :disabled="!selectedAccountId || executing">
          {{ executing ? '执行中...' : '执行签到' }}
        </button>
        <button @click="retryFailedRuns" class="btn-retry" :disabled="failedAccountIds.length === 0 || actionBusy">
          {{ retryingBatch ? '重试中...' : `重试失败账户 ${failedAccountIds.length}` }}
        </button>
        <input v-model.number="keepLatest" type="number" min="0" max="10000" class="keep-input" title="保留最新记录数（0=清除全部）" />
        <button @click="cleanupRuns" class="btn-cleanup" :disabled="cleaning">
          {{ cleaning ? '清理中...' : '清理记录' }}
        </button>
      </div>
    </div>

    <!-- 筛选栏 -->
    <div class="filter-bar">
      <div class="status-filter">
        <button
          v-for="status in statusOptions"
          :key="status.value"
          :class="['status-btn', { active: filterStatus === status.value }]"
          @click="filterStatus = status.value"
        >
          {{ status.label }}
          <span v-if="statusCounts[status.value]" class="count">
            {{ statusCounts[status.value] }}
          </span>
        </button>
      </div>
      <select v-model="filterTriggeredBy" class="filter-select">
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
          placeholder="开始时间"
        />
        <span class="date-separator">至</span>
        <input
          v-model="filterEndDate"
          type="datetime-local"
          class="filter-input"
          placeholder="结束时间"
        />
      </div>
      <select v-model="filterAccountId" class="filter-select">
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

    <div v-if="lastBatchResult" class="batch-result">
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

    <div class="runs-list">
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
      <div v-if="runs.length === 0 && !runsLoading" class="empty">暂无签到记录</div>
      <div v-if="runsLoading" class="empty">加载中...</div>
      <div v-if="hasMore && runs.length > 0 && !runsLoading" class="load-more">
        <button @click="loadMoreRuns">加载更多</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { apiUrl, authHeaders, request, responseData } from '../utils/api'
import { confirmAction, showToast } from '../utils/toast'
import type { CurrentUser, Account, AccountGroup } from '../types'
import { useUsers } from '../composables/useUsers'

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
const runsLoading = ref(false)
const runsOffset = ref(0)
const hasMore = ref(true)
const executing = ref(false)
const executingAccountId = ref('')
const retryingBatch = ref(false)
const cleaning = ref(false)
const PAGE_SIZE = 100
const maxAttemptsPerDay = ref(3)
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
  try {
    let url = apiUrl('/accounts')
    if (props.isAdmin && filterUserId.value) {
      url += `?userId=${encodeURIComponent(filterUserId.value)}`
    }
    const response = await request(url, { headers: authHeaders() })
    accounts.value = await responseData<Account[]>(response)
    // 如果当前选中的账户不在新列表中，清除选择
    if (selectedAccountId.value && !accounts.value.find((a) => a.id === selectedAccountId.value)) {
      selectedAccountId.value = ''
    }
    if (!selectedAccountId.value && accounts.value.length > 0) {
      selectedAccountId.value = accounts.value[0].id
    }
  } catch (error) {
    showToast(error instanceof Error ? error.message : '加载账户失败', 'error')
  }
}

const fetchRuns = async (append = false) => {
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

    const response = await request(url, { headers: authHeaders() })
    const data = await responseData<CheckinRun[]>(response)
    if (append) {
      runs.value.push(...data)
    } else {
      runs.value = data
      runsOffset.value = 0
    }
    runsOffset.value += data.length
    hasMore.value = data.length >= PAGE_SIZE
  } catch (error) {
    showToast(error instanceof Error ? error.message : '加载签到记录失败', 'error')
  } finally {
    runsLoading.value = false
  }
}

const loadMoreRuns = () => fetchRuns(true)

const fetchSettings = async () => {
  try {
    const res = await request(apiUrl('/settings'), { headers: authHeaders() })
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
      headers: { ...authHeaders(), 'Content-Type': 'application/json' },
      body: JSON.stringify({ accountId })
    })
    showToast('签到已执行', 'success')
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
  if (!(await confirmAction(`确定重试当前列表中的 ${accountIds.length} 个失败账户吗？`))) return

  retryingBatch.value = true
  lastBatchResult.value = null
  try {
    const response = await request(apiUrl('/checkin-runs/batch'), {
      method: 'POST',
      headers: { ...authHeaders(), 'Content-Type': 'application/json' },
      body: JSON.stringify({ accountIds })
    })
    const result = await responseData<BatchCheckinResult>(response)
    lastBatchResult.value = result
    showToast(
      `重试完成：成功 ${result.succeeded}，跳过 ${result.skipped}，失败 ${result.failed}`,
      result.failed > 0 ? 'error' : 'success'
    )
    await Promise.all([fetchRuns(), fetchAccounts()])
  } catch (error) {
    showToast(error instanceof Error ? error.message : '重试失败账户失败', 'error')
  } finally {
    retryingBatch.value = false
  }
}

const cleanupRuns = async () => {
  if (cleaning.value) return
  const msg = keepLatest.value === 0
    ? '确定清除全部签到记录吗？此操作不可撤销！'
    : `确定清理记录并保留最新 ${keepLatest.value} 条吗？`
  if (!(await confirmAction(msg))) return
  cleaning.value = true
  try {
    await request(apiUrl('/checkin-runs/cleanup'), {
      method: 'POST',
      headers: { ...authHeaders(), 'Content-Type': 'application/json' },
      body: JSON.stringify({ keepLatest: keepLatest.value })
    })
    showToast('记录已清理', 'success')
    await fetchRuns()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '清理记录失败', 'error')
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
.checkin-runs-panel { max-width: 1200px; margin: 0 auto; padding: 2rem; }
.header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 2rem; gap: 1rem; flex-wrap: wrap; }
.header-actions { display: flex; gap: .75rem; align-items: center; flex-wrap: wrap; }
.user-filter { background: var(--bg-well); color: #fff; border: 1px solid var(--border-input); border-radius: 6px; padding: .5rem .65rem; font-size: .85rem; }
h2 { color: #fff; }
.panel-subtitle { color: var(--text-muted); font-size: 0.9rem; margin-top: 0.25rem; }
select, input { background: var(--bg-well); color: #fff; border: 1px solid var(--border-input); border-radius: 6px; padding: .5rem; }
.keep-input { width: 90px; }
.filter-bar { display: flex; gap: 0.75rem; align-items: center; flex-wrap: wrap; padding: 1rem; background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius); margin-bottom: 1.5rem; }
.filter-select { background: var(--bg-well); border: 1px solid var(--border-input); border-radius: 6px; color: white; padding: 0.5rem 0.75rem; font-size: 0.85rem; }
.filter-input { background: var(--bg-well); border: 1px solid var(--border-input); border-radius: 6px; color: white; padding: 0.5rem 0.75rem; font-size: 0.85rem; min-width: 180px; }
.date-range { display: flex; align-items: center; gap: 0.5rem; }
.date-separator { color: #9ca3af; }
.status-filter { display: flex; gap: 0.5rem; }
.status-btn { background: var(--bg-elevated); border: 1px solid var(--border-strong); border-radius: var(--radius-pill); padding: 0.4rem 0.8rem; font-size: 0.8rem; cursor: pointer; transition: all 0.2s; color: white; }
.status-btn:hover { background: #4b5563; }
.status-btn.active { background: var(--accent); border-color: var(--accent-border); }
.status-btn .count { background: rgba(255, 255, 255, 0.2); border-radius: var(--radius-pill); padding: 0.1rem 0.4rem; margin-left: 0.3rem; font-size: 0.7rem; }
.clear-filter { background: #475569; border: none; border-radius: 6px; padding: 0.5rem 0.75rem; color: white; font-size: 0.8rem; cursor: pointer; }
.clear-filter:hover { background: #9ca3af; }
.filter-count { color: #9ca3af; font-size: 0.85rem; margin-left: auto; }
.summary-grid { display: grid; grid-template-columns: repeat(4, minmax(140px, 1fr)); gap: 0.75rem; margin-bottom: 1.5rem; }
.summary-card { background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius); padding: 1rem; display: grid; gap: 0.25rem; }
.summary-card strong { color: var(--text-strong); font-size: 1.4rem; }
.summary-card span { color: var(--text-muted); font-size: 0.85rem; }
.summary-card.danger strong { color: #f87171; }
.batch-result { background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius); padding: 1rem; margin-bottom: 1.5rem; }
.batch-result-header { display: flex; align-items: center; justify-content: space-between; gap: 1rem; }
.batch-items { display: grid; gap: 0.5rem; margin-top: 0.9rem; max-height: 260px; overflow: auto; }
.batch-item { display: grid; grid-template-columns: minmax(160px, 1fr) auto minmax(160px, 2fr); align-items: center; gap: 0.75rem; padding: 0.55rem 0.65rem; background: var(--bg-well); border: 1px solid var(--border); border-radius: 6px; }
.batch-name,
.batch-message { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.batch-name { color: #e5e7eb; font-weight: 600; }
.batch-message { color: var(--text-muted); }
.runs-list { display: grid; gap: 1.5rem; }
.run-group { display: grid; gap: 0.75rem; }
.group-header { display: flex; align-items: center; gap: 0.6rem; padding-bottom: 0.25rem; border-bottom: 1px solid var(--border); }
.group-header strong { color: #e5e7eb; font-size: 1rem; }
.group-header .muted { font-size: 0.8rem; }
.self-tag { background: var(--accent); border-radius: var(--radius-pill); padding: 0.05rem 0.45rem; margin-left: 0.4rem; font-size: 0.7rem; color: #fff; font-weight: normal; }
.run-card { background: var(--bg-card); padding: 1.2rem; border-radius: var(--radius); border: 1px solid var(--border); border-left: 4px solid #64748b; transition: background-color 0.16s ease, border-color 0.16s ease; display: flex; justify-content: space-between; gap: 1rem; }
.run-card:hover { background: #151f2f; border-color: var(--border-strong); }
.run-card.success { border-left-color: var(--success); }
.run-card.failed { border-left-color: var(--danger); }
.run-card.already_checked { border-left-color: var(--accent-border); }
.run-card.pending { border-left-color: var(--warn); }
.run-info .account-name { color: #fff; font-size: 1.1rem; font-weight: bold; }
.run-info { display: flex; flex-direction: column; gap: 0.5rem; }
.run-title { display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap; }
.run-meta { display: flex; flex-direction: column; gap: 0.25rem; color: var(--text-muted); font-size: 0.9rem; }
.run-actions { display: flex; gap: 0.5rem; align-items: flex-start; flex-wrap: wrap; justify-content: flex-end; min-width: 160px; }
.badge { padding: 0.25rem 0.55rem; border-radius: var(--radius-pill); font-size: 0.75rem; display: inline-block; width: fit-content; background: #475569; color: white; }
.badge.success { background: var(--success); }
.badge.failed { background: var(--danger); }
.badge.already_checked { background: var(--accent-border); }
.badge.pending { background: var(--warn); }
.badge.neutral { background: #475569; }
.site-tag { background: #1e293b; color: var(--text-faint); border: 1px solid var(--border-strong); border-radius: var(--radius-pill); padding: 0.2rem 0.5rem; font-size: 0.75rem; }
button { color: white; border: none; padding: 0.5rem 1rem; border-radius: 6px; cursor: pointer; background: var(--border-input); }
button:hover:not(:disabled) { background: #4b5563; }
button:disabled { background: #555; cursor: not-allowed; opacity: 0.6; }
.btn-execute { background: var(--accent); }
.btn-execute:hover:not(:disabled) { background: var(--accent-hover); }
.btn-retry { background: #0f766e; }
.btn-retry:hover:not(:disabled) { background: #0d9488; }
.btn-cleanup { background: #dc2626; }
.btn-cleanup:hover:not(:disabled) { background: #b91c1c; }
button.ghost { background: transparent; border: 1px solid var(--border-strong); color: var(--text-faint); }
button.ghost:hover:not(:disabled) { background: var(--bg-elevated); }
.empty { text-align: center; color: var(--text-muted); padding: 3rem; background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius); }
.load-more { text-align: center; padding: 1rem; }
.load-more button { background: var(--border-input); color: var(--text-faint); border: 1px solid #4b5563; padding: 0.5rem 1.5rem; border-radius: 6px; cursor: pointer; }
.load-more button:hover { background: #4b5563; color: #fff; }

@media (max-width: 768px) {
  .checkin-runs-panel { padding: 1rem; }
  .header-actions,
  .status-filter,
  .date-range { width: 100%; }
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
