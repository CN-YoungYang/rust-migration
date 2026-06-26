<template>
  <div class="checkin-runs-panel">
    <div class="header">
      <h2>签到记录</h2>
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


    <div class="runs-list">
      <section v-for="group in groupedRuns" :key="group.key" class="run-group">
        <div class="group-header">
          <strong>{{ group.label }}<span v-if="group.isSelf" class="self-tag">我</span></strong>
          <span class="muted">{{ group.items.length }} 条记录</span>
        </div>
        <div v-for="run in group.items" :key="run.id" class="run-card" :class="run.status.toLowerCase()">
          <div class="run-info">
            <span class="account-name">{{ accountName(run.accountId) }}</span>
            <span class="badge" :class="run.status.toLowerCase()">{{ statusText(run.status) }}</span>
            <div class="run-meta">
              <span>触发方式: {{ triggerText(run.triggeredBy) }}</span>
              <span>时间: {{ formatTime(run.createdAt) }}</span>
              <span v-if="run.durationMs">耗时: {{ run.durationMs }}ms</span>
              <span v-if="run.message">消息: {{ run.message }}</span>
            </div>
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
const cleaning = ref(false)
const PAGE_SIZE = 100
const maxAttemptsPerDay = ref(3)

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

const executeCheckin = async () => {
  if (!selectedAccountId.value || executing.value) return
  // 检查是否达到每日上限，达到则弹窗确认
  const account = accounts.value.find((a) => a.id === selectedAccountId.value)
  if (account && (account.todayRuns ?? 0) >= maxAttemptsPerDay.value) {
    const confirmed = await confirmAction(
      `该账户今日已签到 ${account.todayRuns} 次，已达每日上限（${maxAttemptsPerDay.value} 次）。\n手动签到不受限制，确定继续吗？`
    )
    if (!confirmed) return
  }
  executing.value = true
  try {
    await request(apiUrl('/checkin-runs'), {
      method: 'POST',
      headers: { ...authHeaders(), 'Content-Type': 'application/json' },
      body: JSON.stringify({ accountId: selectedAccountId.value })
    })
    showToast('签到已执行', 'success')
    await fetchRuns()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '执行签到失败', 'error')
  } finally {
    executing.value = false
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
  return accounts.value.find((account) => account.id === accountId)?.name || accountId
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
.user-filter { background: #1a2937; color: #fff; border: 1px solid #374151; border-radius: 4px; padding: .4rem .6rem; font-size: .85rem; }
h2 { color: #fff; }
select, input { background: #111827; color: #fff; border: 1px solid #374151; border-radius: 4px; padding: .5rem; }
.keep-input { width: 90px; }
.filter-bar { display: flex; gap: 0.75rem; align-items: center; flex-wrap: wrap; padding: 1rem; background: #1a1a1a; border-radius: 8px; margin-bottom: 1.5rem; }
.filter-select { background: #0b1220; border: 1px solid #374151; border-radius: 4px; color: white; padding: 0.5rem 0.75rem; font-size: 0.85rem; }
.filter-input { background: #0b1220; border: 1px solid #374151; border-radius: 4px; color: white; padding: 0.5rem 0.75rem; font-size: 0.85rem; min-width: 180px; }
.date-range { display: flex; align-items: center; gap: 0.5rem; }
.date-separator { color: #9ca3af; }
.status-filter { display: flex; gap: 0.5rem; }
.status-btn { background: #374151; border: 1px solid #4b5563; border-radius: 4px; padding: 0.4rem 0.8rem; font-size: 0.8rem; cursor: pointer; transition: all 0.2s; color: white; }
.status-btn:hover { background: #4b5563; }
.status-btn.active { background: #0070f3; border-color: #0070f3; }
.status-btn .count { background: rgba(255, 255, 255, 0.2); border-radius: 999px; padding: 0.1rem 0.4rem; margin-left: 0.3rem; font-size: 0.7rem; }
.clear-filter { background: #6b7280; border: none; border-radius: 4px; padding: 0.5rem 0.75rem; color: white; font-size: 0.8rem; cursor: pointer; }
.clear-filter:hover { background: #9ca3af; }
.filter-count { color: #9ca3af; font-size: 0.85rem; margin-left: auto; }
.runs-list { display: grid; gap: 1.5rem; }
.run-group { display: grid; gap: 0.75rem; }
.group-header { display: flex; align-items: center; gap: 0.6rem; padding-bottom: 0.25rem; border-bottom: 1px solid #2a2a2a; }
.group-header strong { color: #e5e7eb; font-size: 1rem; }
.group-header .muted { font-size: 0.8rem; }
.self-tag { background: #0070f3; border-radius: 999px; padding: 0.05rem 0.45rem; margin-left: 0.4rem; font-size: 0.7rem; color: #fff; font-weight: normal; }
.run-card { background: #1a1a1a; padding: 1.5rem; border-radius: 8px; border-left: 4px solid #666; }
.run-card.success { border-left-color: #10b981; }
.run-card.failed { border-left-color: #ef4444; }
.run-card.already_checked { border-left-color: #3b82f6; }
.run-card.pending { border-left-color: #f59e0b; }
.run-info .account-name { color: #fff; font-size: 1.1rem; font-weight: bold; }
.run-info { display: flex; flex-direction: column; gap: 0.5rem; }
.run-meta { display: flex; flex-direction: column; gap: 0.25rem; color: #888; font-size: 0.9rem; }
.badge { padding: 0.25rem 0.5rem; border-radius: 4px; font-size: 0.75rem; display: inline-block; width: fit-content; background: #666; color: white; }
.badge.success { background: #10b981; }
.badge.failed { background: #ef4444; }
.badge.already_checked { background: #3b82f6; }
.badge.pending { background: #f59e0b; }
button { color: white; border: none; padding: 0.5rem 1rem; border-radius: 4px; cursor: pointer; }
button:disabled { background: #555; cursor: not-allowed; opacity: 0.6; }
.btn-execute { background: #0070f3; }
.btn-cleanup { background: #ef4444; }
.empty { text-align: center; color: #666; padding: 3rem; background: #1a1a1a; border-radius: 8px; }
.load-more { text-align: center; padding: 1rem; }
.load-more button { background: #374151; color: #9ca3af; border: 1px solid #4b5563; padding: 0.5rem 1.5rem; border-radius: 4px; cursor: pointer; }
.load-more button:hover { background: #4b5563; color: #fff; }
</style>
