<template>
  <section class="account-panel">
    <div class="panel-header">
      <div>
        <h2>签到账户管理</h2>
        <p class="panel-subtitle">
          已启用 {{ listSummary.enabled }} 个，今日执行 {{ listSummary.todayRuns }} 次，失败 {{ listSummary.failed }} 个
        </p>
      </div>
      <div class="header-actions">
        <select v-if="isAdmin" v-model="filterUserId" class="user-filter" aria-label="按用户筛选账户">
          <option value="">全部用户</option>
          <option v-if="usersLoading" disabled>加载中...</option>
          <option v-for="u in allUsers" :key="u.id" :value="u.id">{{ u.username }}</option>
        </select>
        <button class="secondary" @click="exportAccounts" :disabled="loading || actionBusy">导出 CSV</button>
        <button class="secondary" @click="openImportDialog" :disabled="actionBusy">导入 CSV</button>
        <button
          v-if="accounts.length > 0"
          class="primary"
          :disabled="actionBusy"
          @click="batchCheckin(accounts.map((a) => a.id), filterUserId ? '该用户账户' : '当前列表账户')"
        >
          {{ batchLoading ? '签到中...' : (filterUserId ? '该用户签到' : '当前列表签到') }}
        </button>
        <button class="primary" :disabled="actionBusy" @click="openCreate">新增账户</button>
      </div>
    </div>

    <div class="filter-bar">
      <select v-model="filterSiteType" class="filter-select" aria-label="按站点类型筛选">
        <option value="">全部类型</option>
        <option value="new-api">new-api</option>
        <option value="anyrouter">anyrouter</option>
        <option value="x666">x666</option>
      </select>
      <select v-model="filterEnabled" class="filter-select" aria-label="按启用状态筛选">
        <option value="">全部状态</option>
        <option value="true">已启用</option>
        <option value="false">已禁用</option>
      </select>
      <select v-model="filterLastStatus" class="filter-select" aria-label="按签到状态筛选">
        <option value="">全部签到状态</option>
        <option value="not_today">今日未签到</option>
        <option value="success">成功</option>
        <option value="failed">失败</option>
        <option value="already_checked">今日已签</option>
        <option value="never">从未签到</option>
      </select>
      <input
        v-model="filterKeyword"
        type="search"
        aria-label="搜索账户"
        placeholder="搜索账户名称、地址或备注"
        class="filter-input"
      />
      <button v-if="hasActiveFilter" class="clear-filter" @click="clearFilters">清除筛选</button>
      <span class="filter-count">{{ accounts.length }} 个结果</span>
    </div>

    <div v-if="!loading && accounts.length > 0" class="bulk-toolbar">
      <label class="select-all">
        <input
          type="checkbox"
          :checked="allVisibleSelected"
          :indeterminate.prop="someVisibleSelected"
          @change="toggleSelectAllVisible"
        />
        选中本页
      </label>
      <span class="selection-count">已选 {{ selectedIds.length }} 个</span>
      <button :disabled="selectedIds.length === 0 || actionBusy" @click="batchCheckin(selectedIds, '选中账户')">
        签到选中
      </button>
      <button :disabled="selectedIds.length === 0 || actionBusy" @click="bulkRefreshBalance">
        刷新余额
      </button>
      <button :disabled="selectedIds.length === 0 || actionBusy" @click="bulkSetEnabled(true)">
        批量启用
      </button>
      <button :disabled="selectedIds.length === 0 || actionBusy" @click="bulkSetEnabled(false)">
        批量禁用
      </button>
      <button v-if="selectedIds.length > 0" class="ghost" :disabled="actionBusy" @click="clearSelection">
        清空选择
      </button>
    </div>

    <div v-if="bulkProgress" class="progress-panel" role="status" aria-live="polite" aria-atomic="true">
      <div class="progress-meta">
        <strong>{{ bulkProgress.label }}</strong>
        <span>{{ bulkProgress.completed }} / {{ bulkProgress.total }}</span>
      </div>
      <div class="progress-track" role="progressbar" :aria-label="bulkProgress.label" aria-valuemin="0" :aria-valuemax="bulkProgress.total" :aria-valuenow="bulkProgress.completed">
        <span :style="{ width: progressPercent + '%' }"></span>
      </div>
      <p v-if="bulkProgress.current" class="muted">当前：{{ bulkProgress.current }}</p>
    </div>

    <div v-if="bulkErrors.length > 0" class="error-panel" role="alert" aria-live="assertive">
      <div class="error-panel-header">
        <strong>失败摘要</strong>
        <button class="ghost" @click="bulkErrors = []">清除</button>
      </div>
      <ul>
        <li v-for="err in bulkErrors" :key="err">{{ err }}</li>
      </ul>
    </div>

    <div v-if="lastBatchResult" class="batch-result" role="status" aria-live="polite">
      <div class="batch-result-header">
        <div>
          <strong>批量签到结果</strong>
          <p class="muted">
            共 {{ lastBatchResult.total }} 个，成功 {{ lastBatchResult.succeeded }} 个，跳过 {{ lastBatchResult.skipped }} 个，失败 {{ lastBatchResult.failed }} 个
          </p>
        </div>
        <button class="ghost" @click="lastBatchResult = null">关闭</button>
      </div>
      <div class="batch-items">
        <div v-for="item in lastBatchResult.items" :key="item.accountId" class="batch-item">
          <span class="batch-name">{{ item.accountName }}</span>
          <span class="status-pill" :class="batchStatusClass(item.status)">{{ batchStatusText(item.status) }}</span>
          <span v-if="item.message" class="batch-message" :title="item.message">{{ item.message }}</span>
        </div>
      </div>
    </div>

    <p v-if="loading" class="empty" role="status" aria-live="polite">加载中...</p>

    <div v-if="!loading" class="account-list">
      <section v-for="group in groupedAccounts" :key="group.key" class="account-group">
        <div class="group-header">
          <h3>{{ group.label }}<span v-if="group.isSelf" class="self-tag">我</span></h3>
          <span class="muted">{{ group.items.length }} 个账户</span>
          <button
            class="batch-btn"
            :disabled="actionBusy"
            @click="batchCheckin(group.items.map((a) => a.id), `${group.label} 分组`)"
          >
            {{ batchLoading ? '执行中...' : '该组签到' }}
          </button>
        </div>

        <article
          v-for="account in group.items"
          :key="account.id"
          class="account-card"
          :class="{ selected: selectedAccountIds.has(account.id), disabled: !account.enabled }"
        >
          <label class="card-select" :title="selectedAccountIds.has(account.id) ? '取消选择' : '选择账户'">
            <input
              type="checkbox"
              :checked="selectedAccountIds.has(account.id)"
              @change="toggleAccountSelection(account.id, $event)"
            />
          </label>

          <div class="account-main">
            <div class="title-row">
              <strong>{{ account.name }}</strong>
              <span class="badge">{{ account.siteType }}</span>
              <span v-if="!account.enabled" class="badge disabled">已禁用</span>
              <span class="status-pill" :class="accountStatusClass(account.lastStatus)">
                {{ accountStatusText(account.lastStatus) }}
              </span>
              <span v-if="accountCheckedToday(account)" class="status-pill today">
                今日 {{ account.todayRuns ?? 0 }} 次
              </span>
            </div>

            <div class="meta-grid">
              <span><b>地址</b>{{ account.baseUrl || '-' }}</span>
              <span><b>认证</b>{{ account.authType || '-' }}</span>
              <span><b>余额</b>{{ formatBalance(account.lastBalance) }}</span>
              <span><b>最近签到</b>{{ formatDateTime(account.lastRunAt) }}</span>
              <span v-if="account.ownerName"><b>归属</b>{{ account.ownerName }}</span>
              <span v-if="account.lastBalanceAt"><b>余额刷新</b>{{ formatDateTime(account.lastBalanceAt) }}</span>
            </div>

            <p v-if="account.lastMessage" class="message" :title="account.lastMessage">
              {{ account.lastMessage }}
            </p>
            <p v-if="account.note" class="note">备注：{{ account.note }}</p>
          </div>

          <div class="actions">
            <button @click="refreshBalance(account.id)" :disabled="isAccountBusy(account.id)">
              {{ isAccountProcessing(account.id) ? '处理中...' : '刷新余额' }}
            </button>
            <button @click="toggleAccountEnabled(account)" :disabled="isAccountBusy(account.id)">
              {{ account.enabled ? '禁用' : '启用' }}
            </button>
            <button @click="openEdit(account)" :disabled="actionBusy">编辑</button>
            <button class="danger" @click="deleteAccount(account.id)" :disabled="actionBusy">删除</button>
          </div>
        </article>
      </section>
      <p v-if="accounts.length === 0" class="empty" role="status">暂无账户，可使用右上角“新增账户”开始配置。</p>
    </div>

    <div v-if="showForm" class="modal" role="presentation" @click.self="closeForm" @keydown.escape="closeForm">
      <form v-focus-trap class="modal-content" role="dialog" aria-modal="true" aria-labelledby="account-form-title" tabindex="-1" @submit.prevent="submitForm">
        <h3 id="account-form-title">{{ editingId ? '编辑账户' : '新增账户' }}</h3>
        <label>名称<input v-model="form.name" required /></label>
        <label>站点类型
          <select v-model="form.siteType" :disabled="Boolean(editingId)">
            <option value="new-api">new-api</option>
            <option value="anyrouter">anyrouter</option>
            <option value="x666">x666</option>
          </select>
        </label>
        <label>站点地址<input v-model="form.baseUrl" required /></label>
        <label>用户ID<input v-model="form.userId" /></label>
        <label>认证方式
          <select v-model="form.authType" :disabled="Boolean(editingId)">
            <option value="access_token">access_token</option>
            <option value="cookie">cookie</option>
          </select>
        </label>
        <label>Access Token<input v-model="form.accessToken" type="password" autocomplete="new-password" /></label>
        <label>Cookie<textarea v-model="form.cookie" rows="3"></textarea></label>
        <label>自定义签到URL<input v-model="form.customCheckinUrl" /></label>
        <label class="inline"><input v-model="form.enabled" type="checkbox" /> 启用</label>
        <label class="inline"><input v-model="form.retryEnabled" type="checkbox" /> 允许重试</label>
        <label>备注<input v-model="form.note" placeholder="可选，方便识别账户" /></label>
        <div class="modal-actions">
          <button class="primary" type="submit" :disabled="formSubmitting">
            {{ formSubmitting ? '保存中...' : '保存' }}
          </button>
          <button type="button" @click="closeForm" :disabled="formSubmitting">取消</button>
        </div>
      </form>
    </div>

    <div v-if="showImportDialog" class="modal" role="presentation" @click.self="closeImportDialog" @keydown.escape="closeImportDialog">
      <div v-focus-trap class="modal-content import-dialog" role="dialog" aria-modal="true" aria-labelledby="import-dialog-title" tabindex="-1">
        <h3 id="import-dialog-title">批量导入账户</h3>
        <p class="muted">支持 CSV 格式，需包含 header 行</p>

        <div class="import-instructions">
          <h4>CSV 格式说明</h4>
          <p>必填字段：name, siteType, baseUrl, authType</p>
          <p>可选字段：userId, accessToken, cookie, customCheckinUrl, enabled, retryEnabled, note</p>
          <details>
            <summary>查看示例</summary>
            <pre>name,siteType,baseUrl,authType,accessToken,cookie,enabled
测试账户,new-api,https://api.example.com,access_token,sk-xxx,,true</pre>
          </details>
        </div>

        <input
          type="file"
          accept=".csv"
          aria-label="选择账户 CSV 文件"
          @change="handleFileSelect"
          class="file-input"
        />

        <div v-if="importResult" class="import-result">
          <p class="success" v-if="importResult.success > 0">成功导入 {{ importResult.success }} 个账户</p>
          <p class="error" v-if="importResult.failed > 0">失败 {{ importResult.failed }} 个</p>
          <div v-if="importResult.errors.length > 0" class="error-list">
            <details>
              <summary>查看错误详情</summary>
              <ul>
                <li v-for="(err, idx) in importResult.errors" :key="idx">{{ err }}</li>
              </ul>
            </details>
          </div>
        </div>

        <div class="modal-actions">
          <button type="button" @click="closeImportDialog" :disabled="importing">关闭</button>
          <button type="button" class="primary" @click="executeImport" :disabled="!selectedFile || importing">
            {{ importing ? '导入中...' : '开始导入' }}
          </button>
        </div>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref, watch } from 'vue'
import { apiUrl, authHeaders, request, responseData } from '../utils/api'
import { confirmAction, showToast } from '../utils/toast'
import { vFocusTrap } from '../utils/dialogFocus'
import type { CurrentUser, Account, AccountGroup } from '../types'
import { useUsers } from '../composables/useUsers'

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

interface BulkProgress {
  label: string
  completed: number
  total: number
  current?: string
}

const props = defineProps<{
  currentUser: CurrentUser | null
  isAdmin: boolean
}>()

const { allUsers, usersLoading, loadUsers } = useUsers(() => props.isAdmin)
const filterUserId = ref('')
const filterSiteType = ref('')
const filterEnabled = ref('')
const filterLastStatus = ref('')
const filterKeyword = ref('')

const QUOTA_PER_USD = 500000

const accounts = ref<Account[]>([])
const loading = ref(false)
const showForm = ref(false)
const editingId = ref('')
const batchLoading = ref(false)
const bulkLoading = ref(false)
const formSubmitting = ref(false)
const busyAccountIds = ref<Set<string>>(new Set())
const selectedAccountIds = ref<Set<string>>(new Set())
const bulkProgress = ref<BulkProgress | null>(null)
const bulkErrors = ref<string[]>([])
const lastBatchResult = ref<BatchCheckinResult | null>(null)
let accountRequestSeq = 0

const actionBusy = computed(() => (
  batchLoading.value
  || bulkLoading.value
  || formSubmitting.value
  || busyAccountIds.value.size > 0
))

const visibleAccountIds = computed(() => accounts.value.map((account) => account.id))
const selectedIds = computed(() => visibleAccountIds.value.filter((id) => selectedAccountIds.value.has(id)))
const allVisibleSelected = computed(() => (
  visibleAccountIds.value.length > 0
  && visibleAccountIds.value.every((id) => selectedAccountIds.value.has(id))
))
const someVisibleSelected = computed(() => selectedIds.value.length > 0 && !allVisibleSelected.value)

const listSummary = computed(() => {
  let enabled = 0
  let failed = 0
  let todayRuns = 0
  for (const account of accounts.value) {
    if (account.enabled) enabled += 1
    if (account.lastStatus === 'failed') failed += 1
    todayRuns += account.todayRuns ?? 0
  }
  return { enabled, failed, todayRuns }
})

const progressPercent = computed(() => {
  if (!bulkProgress.value || bulkProgress.value.total === 0) return 0
  return Math.min(100, Math.round((bulkProgress.value.completed / bulkProgress.value.total) * 100))
})

const hasActiveFilter = computed(() => {
  return !!(filterSiteType.value || filterEnabled.value || filterLastStatus.value || filterKeyword.value)
})

const groupedAccounts = computed<AccountGroup[]>(() => {
  const groups = new Map<string, AccountGroup>()
  for (const account of accounts.value) {
    const key = account.ownerId || 'unknown'
    if (!groups.has(key)) {
      const label = account.ownerName || (account.ownerId ? `用户 ${account.ownerId.slice(0, 8)}` : '未知用户')
      groups.set(key, {
        key,
        label,
        isSelf: !!props.currentUser && account.ownerId === props.currentUser.id,
        items: [],
      })
    }
    groups.get(key)!.items.push(account)
  }
  return Array.from(groups.values()).sort((a, b) => {
    if (a.isSelf !== b.isSelf) return a.isSelf ? -1 : 1
    return a.label.localeCompare(b.label, 'zh-Hans')
  })
})

const form = reactive({
  name: '',
  siteType: 'new-api',
  baseUrl: '',
  userId: '',
  authType: 'access_token',
  accessToken: '',
  cookie: '',
  customCheckinUrl: '',
  enabled: true,
  retryEnabled: true,
  note: '',
})

function formatBalance(value: number | string | null | undefined): string {
  if (value === null || value === undefined || value === '') return '未刷新'
  const quota = typeof value === 'string' ? parseFloat(value) : value
  if (!Number.isFinite(quota)) return '未刷新'
  return `$${(quota / QUOTA_PER_USD).toFixed(2)}`
}

function formatDateTime(value: string | null | undefined): string {
  if (!value) return '无记录'
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return '无记录'
  return date.toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

function accountStatusText(status: string | null | undefined): string {
  const map: Record<string, string> = {
    success: '成功',
    failed: '失败',
    already_checked: '今日已签',
    pending: '进行中',
  }
  return status ? (map[status] || status) : '未签到'
}

function accountStatusClass(status: string | null | undefined): string {
  if (!status) return 'neutral'
  if (status === 'already_checked') return 'already'
  return status
}

function batchStatusText(status: string): string {
  const map: Record<string, string> = {
    success: '成功',
    failed: '失败',
    skipped: '跳过',
    already_checked: '今日已签',
    pending: '进行中',
  }
  return map[status] || status
}

function batchStatusClass(status: string): string {
  if (status === 'already_checked') return 'already'
  if (status === 'skipped') return 'neutral'
  return status
}

function accountCheckedToday(account: Account): boolean {
  if ((account.todayRuns ?? 0) > 0) return true
  if (!account.lastRunAt) return false
  const runDate = new Date(account.lastRunAt)
  if (Number.isNaN(runDate.getTime())) return false
  const today = new Date()
  return runDate.toDateString() === today.toDateString()
}

function clearFilters() {
  filterSiteType.value = ''
  filterEnabled.value = ''
  filterLastStatus.value = ''
  filterKeyword.value = ''
}

function pruneSelection() {
  const visible = new Set(visibleAccountIds.value)
  selectedAccountIds.value = new Set([...selectedAccountIds.value].filter((id) => visible.has(id)))
}

function toggleAccountSelection(id: string, event: Event) {
  const checked = (event.target as HTMLInputElement).checked
  const next = new Set(selectedAccountIds.value)
  if (checked) {
    next.add(id)
  } else {
    next.delete(id)
  }
  selectedAccountIds.value = next
}

function toggleSelectAllVisible(event: Event) {
  const checked = (event.target as HTMLInputElement).checked
  const next = new Set(selectedAccountIds.value)
  for (const id of visibleAccountIds.value) {
    if (checked) {
      next.add(id)
    } else {
      next.delete(id)
    }
  }
  selectedAccountIds.value = next
}

function clearSelection() {
  selectedAccountIds.value = new Set()
}

function setAccountBusy(id: string, busy: boolean) {
  const next = new Set(busyAccountIds.value)
  if (busy) {
    next.add(id)
  } else {
    next.delete(id)
  }
  busyAccountIds.value = next
}

function isAccountBusy(id: string): boolean {
  return actionBusy.value || busyAccountIds.value.has(id)
}

function isAccountProcessing(id: string): boolean {
  return busyAccountIds.value.has(id)
}

function resetForm() {
  Object.assign(form, {
    name: '',
    siteType: 'new-api',
    baseUrl: '',
    userId: '',
    authType: 'access_token',
    accessToken: '',
    cookie: '',
    customCheckinUrl: '',
    enabled: true,
    retryEnabled: true,
    note: '',
  })
}

async function loadAccounts() {
  const seq = ++accountRequestSeq
  loading.value = true
  try {
    let url = apiUrl('/accounts')
    const params = new URLSearchParams()

    if (props.isAdmin && filterUserId.value) params.append('userId', filterUserId.value)
    if (filterSiteType.value) params.append('siteType', filterSiteType.value)
    if (filterEnabled.value) params.append('enabled', filterEnabled.value)
    if (filterLastStatus.value) params.append('lastStatus', filterLastStatus.value)
    if (filterKeyword.value) params.append('keyword', filterKeyword.value)

    if (params.toString()) url += `?${params.toString()}`

    const response = await request(url, { headers: authHeaders() })
    const data = await responseData<Account[]>(response)
    if (seq === accountRequestSeq) {
      accounts.value = data
      pruneSelection()
    }
  } catch (error) {
    if (seq === accountRequestSeq) {
      showToast(error instanceof Error ? error.message : '加载账户失败', 'error')
    }
  } finally {
    if (seq === accountRequestSeq) {
      loading.value = false
    }
  }
}

async function batchCheckin(accountIds: readonly string[], scopeLabel = '账户') {
  const ids = [...new Set(accountIds)]
  if (ids.length === 0 || batchLoading.value) return
  if (!(await confirmAction(`确定要对${scopeLabel}（${ids.length} 个）执行签到吗？`))) return

  batchLoading.value = true
  bulkErrors.value = []
  lastBatchResult.value = null
  bulkProgress.value = {
    label: '批量签到',
    completed: 0,
    total: ids.length,
    current: '后端正在按设置串行执行',
  }

  try {
    const response = await request(apiUrl('/checkin-runs/batch'), {
      method: 'POST',
      headers: { ...authHeaders(), 'Content-Type': 'application/json' },
      body: JSON.stringify({ accountIds: ids }),
    })
    const result = await responseData<BatchCheckinResult>(response)
    lastBatchResult.value = result
    bulkProgress.value = {
      label: '批量签到',
      completed: result.total,
      total: result.total,
      current: '已完成',
    }
    showToast(
      `批量签到完成：成功 ${result.succeeded}，跳过 ${result.skipped}，失败 ${result.failed}`,
      result.failed > 0 ? 'error' : 'success',
    )
    await loadAccounts()
  } catch (error) {
    bulkErrors.value = [error instanceof Error ? error.message : '批量签到失败']
    showToast(bulkErrors.value[0], 'error')
  } finally {
    batchLoading.value = false
  }
}

function openCreate() {
  editingId.value = ''
  resetForm()
  showForm.value = true
}

function openEdit(account: Account) {
  editingId.value = account.id
  Object.assign(form, {
    name: account.name,
    siteType: account.siteType,
    baseUrl: account.baseUrl || '',
    userId: account.userId || '',
    authType: account.authType || 'access_token',
    accessToken: '',
    cookie: '',
    customCheckinUrl: account.customCheckinUrl || '',
    enabled: account.enabled ?? true,
    retryEnabled: account.retryEnabled ?? true,
    note: account.note || '',
  })
  showForm.value = true
}

function closeForm() {
  if (formSubmitting.value) return
  showForm.value = false
  editingId.value = ''
}

async function submitForm() {
  if (formSubmitting.value) return
  formSubmitting.value = true
  const optionalString = (value: string) => {
    const trimmed = value.trim()
    if (trimmed) return trimmed
    return editingId.value ? null : undefined
  }
  const payload = {
    name: form.name,
    siteType: form.siteType,
    baseUrl: form.baseUrl,
    userId: optionalString(form.userId),
    authType: form.authType,
    accessToken: form.accessToken.trim() || undefined,
    cookie: form.cookie.trim() || undefined,
    customCheckinUrl: optionalString(form.customCheckinUrl),
    enabled: form.enabled,
    retryEnabled: form.retryEnabled,
    note: optionalString(form.note),
  }

  try {
    await request(editingId.value ? apiUrl(`/accounts/${editingId.value}`) : apiUrl('/accounts'), {
      method: editingId.value ? 'PUT' : 'POST',
      headers: { ...authHeaders(), 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    })
    showToast('保存成功', 'success')
    showForm.value = false
    editingId.value = ''
    await loadAccounts()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '保存失败', 'error')
  } finally {
    formSubmitting.value = false
  }
}

async function deleteAccount(id: string) {
  if (!(await confirmAction('确定要删除此账户吗？'))) return
  try {
    await request(apiUrl(`/accounts/${id}`), { method: 'DELETE', headers: authHeaders() })
    showToast('删除成功', 'success')
    const next = new Set(selectedAccountIds.value)
    next.delete(id)
    selectedAccountIds.value = next
    await loadAccounts()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '删除失败', 'error')
  }
}

async function refreshBalance(id: string) {
  if (isAccountBusy(id)) return
  setAccountBusy(id, true)
  try {
    await request(apiUrl(`/accounts/${id}/refresh-balance`), { method: 'POST', headers: authHeaders() })
    showToast('余额已刷新', 'success')
    await loadAccounts()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '刷新余额失败', 'error')
  } finally {
    setAccountBusy(id, false)
  }
}

async function updateAccountEnabled(id: string, enabled: boolean) {
  await request(apiUrl(`/accounts/${id}`), {
    method: 'PUT',
    headers: { ...authHeaders(), 'Content-Type': 'application/json' },
    body: JSON.stringify({ enabled }),
  })
}

async function toggleAccountEnabled(account: Account) {
  if (isAccountBusy(account.id)) return
  const nextEnabled = !account.enabled
  if (!nextEnabled && !(await confirmAction('禁用后该账户不会参与自动签到，确定继续吗？'))) return

  setAccountBusy(account.id, true)
  try {
    await updateAccountEnabled(account.id, nextEnabled)
    showToast(nextEnabled ? '账户已启用' : '账户已禁用', 'success')
    await loadAccounts()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '更新账户状态失败', 'error')
  } finally {
    setAccountBusy(account.id, false)
  }
}

async function bulkRefreshBalance() {
  const ids = selectedIds.value.slice()
  if (ids.length === 0 || bulkLoading.value) return
  if (!(await confirmAction(`确定刷新选中 ${ids.length} 个账户的余额吗？`))) return

  bulkLoading.value = true
  bulkErrors.value = []
  let succeeded = 0
  let failed = 0

  try {
    for (const [index, id] of ids.entries()) {
      const account = accounts.value.find((item) => item.id === id)
      bulkProgress.value = {
        label: '批量刷新余额',
        completed: index,
        total: ids.length,
        current: account?.name || id,
      }
      setAccountBusy(id, true)
      try {
        await request(apiUrl(`/accounts/${id}/refresh-balance`), { method: 'POST', headers: authHeaders() })
        succeeded += 1
      } catch (error) {
        failed += 1
        const message = error instanceof Error ? error.message : '刷新失败'
        bulkErrors.value.push(`${account?.name || id}：${message}`)
      } finally {
        setAccountBusy(id, false)
      }
    }
    bulkProgress.value = {
      label: '批量刷新余额',
      completed: ids.length,
      total: ids.length,
      current: '已完成',
    }
    showToast(`余额刷新完成：成功 ${succeeded}，失败 ${failed}`, failed > 0 ? 'error' : 'success')
    await loadAccounts()
  } finally {
    bulkLoading.value = false
  }
}

async function bulkSetEnabled(enabled: boolean) {
  const ids = selectedIds.value.slice()
  if (ids.length === 0 || bulkLoading.value) return
  const verb = enabled ? '启用' : '禁用'
  if (!(await confirmAction(`确定${verb}选中 ${ids.length} 个账户吗？`))) return

  bulkLoading.value = true
  bulkErrors.value = []
  let succeeded = 0
  let failed = 0

  try {
    for (const [index, id] of ids.entries()) {
      const account = accounts.value.find((item) => item.id === id)
      bulkProgress.value = {
        label: `批量${verb}`,
        completed: index,
        total: ids.length,
        current: account?.name || id,
      }
      setAccountBusy(id, true)
      try {
        await updateAccountEnabled(id, enabled)
        succeeded += 1
      } catch (error) {
        failed += 1
        const message = error instanceof Error ? error.message : `${verb}失败`
        bulkErrors.value.push(`${account?.name || id}：${message}`)
      } finally {
        setAccountBusy(id, false)
      }
    }
    bulkProgress.value = {
      label: `批量${verb}`,
      completed: ids.length,
      total: ids.length,
      current: '已完成',
    }
    showToast(`${verb}完成：成功 ${succeeded}，失败 ${failed}`, failed > 0 ? 'error' : 'success')
    await loadAccounts()
  } finally {
    bulkLoading.value = false
  }
}

const showImportDialog = ref(false)
const selectedFile = ref<File | null>(null)
const importing = ref(false)
const importResult = ref<{
  success: number
  failed: number
  errors: string[]
} | null>(null)

function openImportDialog() {
  showImportDialog.value = true
  importResult.value = null
  selectedFile.value = null
}

function closeImportDialog() {
  if (importing.value) return
  showImportDialog.value = false
  importResult.value = null
  selectedFile.value = null
}

function handleFileSelect(event: Event) {
  const target = event.target as HTMLInputElement
  if (target.files && target.files.length > 0) {
    selectedFile.value = target.files[0]
    importResult.value = null
  }
}

async function executeImport() {
  if (!selectedFile.value) return

  importing.value = true
  try {
    const csvContent = await selectedFile.value.text()
    const response = await request(apiUrl('/accounts/import'), {
      method: 'POST',
      headers: {
        ...authHeaders(),
        'Content-Type': 'text/csv',
      },
      body: csvContent,
    })

    importResult.value = await responseData<{
      success: number
      failed: number
      errors: string[]
    }>(response)

    if (importResult.value.success > 0) {
      showToast(`成功导入 ${importResult.value.success} 个账户`, 'success')
      await loadAccounts()
    }

    if (importResult.value.failed > 0) {
      showToast(`${importResult.value.failed} 个账户导入失败`, 'error')
    }
  } catch (error) {
    showToast(error instanceof Error ? error.message : '导入失败', 'error')
  } finally {
    importing.value = false
  }
}

async function exportAccounts() {
  try {
    const response = await request(apiUrl('/accounts/export'), {
      headers: authHeaders(),
    })

    const blob = await response.blob()
    const url = window.URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `ai-hub-accounts-${new Date().toISOString().slice(0, 10)}.csv`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    window.URL.revokeObjectURL(url)

    showToast('导出成功', 'success')
  } catch (error) {
    showToast(error instanceof Error ? error.message : '导出失败', 'error')
  }
}

onMounted(() => {
  loadAccounts()
  loadUsers()
})

watch(filterUserId, () => loadAccounts())
watch([filterSiteType, filterEnabled, filterLastStatus], () => loadAccounts())

let keywordDebounce: ReturnType<typeof setTimeout> | null = null
watch(filterKeyword, () => {
  if (keywordDebounce) clearTimeout(keywordDebounce)
  keywordDebounce = setTimeout(() => loadAccounts(), 300)
})

onUnmounted(() => {
  if (keywordDebounce) clearTimeout(keywordDebounce)
})
</script>

<style scoped>
.account-panel { max-width: 1220px; margin: 0 auto; padding: 2rem; }
.panel-header { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 1.25rem; flex-wrap: wrap; gap: .75rem; }
.panel-header h2 { color: var(--text-strong); margin-bottom: 0.25rem; }
.panel-subtitle { color: var(--text-muted); font-size: 0.9rem; }
.header-actions { display: flex; gap: .5rem; align-items: center; flex-wrap: wrap; }
.user-filter { background: var(--bg-well); color: #fff; border: 1px solid var(--border-input); border-radius: 6px; padding: .5rem .65rem; font-size: .85rem; }
.filter-bar,
.bulk-toolbar,
.progress-panel,
.batch-result,
.error-panel {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius);
}
.filter-bar { display: flex; gap: 0.75rem; align-items: center; flex-wrap: wrap; padding: 1rem; margin-bottom: 1rem; }
.filter-select { background: var(--bg-well); border: 1px solid var(--border-input); border-radius: 6px; color: white; padding: 0.5rem 0.75rem; font-size: 0.85rem; }
.filter-input { background: var(--bg-well); border: 1px solid var(--border-input); border-radius: 6px; color: white; padding: 0.5rem 0.75rem; font-size: 0.85rem; min-width: 240px; }
.clear-filter { background: #475569; }
.filter-count { color: #9ca3af; font-size: 0.85rem; margin-left: auto; }
.bulk-toolbar { display: flex; align-items: center; gap: 0.65rem; flex-wrap: wrap; padding: 0.8rem 1rem; margin-bottom: 1rem; }
.select-all { color: var(--text); display: inline-flex; align-items: center; gap: 0.45rem; }
.selection-count { color: var(--text-faint); font-size: 0.9rem; margin-right: auto; }
.progress-panel { padding: 1rem; margin-bottom: 1rem; }
.progress-meta,
.batch-result-header,
.error-panel-header { display: flex; align-items: center; justify-content: space-between; gap: 1rem; }
.progress-track { height: 8px; background: var(--bg-well); border-radius: var(--radius-pill); overflow: hidden; margin: 0.75rem 0 0.35rem; }
.progress-track span { display: block; height: 100%; background: var(--accent); transition: width 0.2s ease; }
.error-panel { padding: 1rem; margin-bottom: 1rem; border-color: rgba(239, 68, 68, 0.45); }
.error-panel ul { margin-top: 0.75rem; padding-left: 1.2rem; color: #fca5a5; }
.batch-result { padding: 1rem; margin-bottom: 1rem; }
.batch-items { display: grid; gap: 0.5rem; margin-top: 0.9rem; max-height: 260px; overflow: auto; }
.batch-item { display: grid; grid-template-columns: minmax(160px, 1fr) auto minmax(160px, 2fr); align-items: center; gap: 0.75rem; padding: 0.55rem 0.65rem; background: var(--bg-well); border: 1px solid var(--border); border-radius: 6px; }
.batch-name { color: #e5e7eb; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.batch-message { color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.account-list { display: grid; gap: 1.5rem; }
.account-group { display: grid; gap: 1rem; }
.group-header { display: flex; align-items: center; gap: .6rem; padding-bottom: .25rem; border-bottom: 1px solid var(--border); }
.group-header h3 { margin: 0; font-size: 1rem; color: #e5e7eb; }
.group-header .muted { font-size: .8rem; flex: 1; }
.group-header .batch-btn { background: #0f766e; font-size: .8rem; padding: .35rem .7rem; }
.self-tag { background: var(--accent); border-radius: var(--radius-pill); padding: .05rem .45rem; margin-left: .4rem; font-size: .7rem; color: #fff; }
.account-card { background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius); padding: 1rem; display: grid; grid-template-columns: auto minmax(0, 1fr) auto; gap: 1rem; transition: border-color 0.16s ease, background-color 0.16s ease; }
.account-card:hover { background: #151f2f; border-color: var(--border-strong); }
.account-card.selected { border-color: var(--accent-border); }
.account-card.disabled { opacity: 0.78; }
.card-select { display: flex; align-items: flex-start; padding-top: 0.25rem; }
.account-main { min-width: 0; }
.title-row { display: flex; gap: .5rem; align-items: center; margin-bottom: .65rem; flex-wrap: wrap; }
.title-row strong { color: var(--text-strong); font-size: 1rem; overflow-wrap: anywhere; }
.badge,
.status-pill { border-radius: var(--radius-pill); padding: .15rem .5rem; font-size: .75rem; white-space: nowrap; }
.badge { background: var(--accent-hover); color: #dbeafe; }
.badge.disabled { background: #475569; color: var(--text-faint); }
.status-pill { background: #475569; color: #e5e7eb; }
.status-pill.success { background: var(--success-soft); color: #34d399; }
.status-pill.failed { background: var(--danger-soft); color: #f87171; }
.status-pill.already { background: rgba(59, 130, 246, 0.18); color: #93c5fd; }
.status-pill.pending { background: rgba(245, 158, 11, 0.18); color: #fbbf24; }
.status-pill.today { background: rgba(20, 184, 166, 0.18); color: #5eead4; }
.status-pill.neutral { background: var(--border-strong); color: var(--text-faint); }
.meta-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 0.35rem 1rem; color: #9ca3af; font-size: 0.88rem; }
.meta-grid span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.meta-grid b { color: var(--text-faint); font-weight: 600; margin-right: 0.4rem; }
.message { margin-top: 0.65rem; color: var(--text-faint); font-size: 0.86rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.note { color: #fbbf24; margin-top: .35rem; font-size: .85rem; overflow-wrap: anywhere; }
.muted { color: #9ca3af; margin: .25rem 0; }
.actions { display: flex; gap: .5rem; align-items: center; flex-wrap: wrap; justify-content: flex-end; align-self: start; max-width: 260px; }
button { border: 0; border-radius: 6px; padding: .5rem .75rem; cursor: pointer; background: var(--border-input); color: white; }
button:hover:not(:disabled) { background: #4b5563; }
button:disabled { opacity: 0.6; cursor: not-allowed; }
button.primary, .primary { background: var(--accent); }
button.primary:hover:not(:disabled), .primary:hover:not(:disabled) { background: var(--accent-hover); }
button.secondary { background: #475569; }
button.secondary:hover:not(:disabled) { background: #64748b; }
button.ghost { background: transparent; border: 1px solid var(--border-strong); color: var(--text-faint); }
button.ghost:hover:not(:disabled) { background: var(--bg-elevated); }
button.danger { background: #dc2626; }
button.danger:hover:not(:disabled) { background: #b91c1c; }
.empty { color: #9ca3af; text-align: center; padding: 2rem; }
.modal { position: fixed; inset: 0; background: rgba(0,0,0,.75); display: flex; align-items: center; justify-content: center; z-index: 20; padding: 1rem; }
.modal-content { width: min(560px, 92vw); max-height: 90vh; overflow: auto; background: var(--bg-card); border: 1px solid var(--border-input); border-radius: var(--radius); padding: 1.5rem; display: grid; gap: .8rem; }
.modal-content h3 { color: var(--text-strong); }
label { display: grid; gap: .35rem; color: var(--text); }
label.inline { display: flex; align-items: center; gap: .5rem; }
input, select, textarea { background: var(--bg-well); border: 1px solid var(--border-input); border-radius: 6px; color: white; padding: .55rem; min-width: 0; }
.modal-actions { display: flex; gap: .75rem; justify-content: flex-end; margin-top: .5rem; }
.import-dialog { max-width: 600px; }
.import-instructions { background: var(--bg-elevated); border: 1px solid var(--border-input); border-radius: 6px; padding: 1rem; margin: 1rem 0; }
.import-instructions h4 { margin: 0 0 0.5rem 0; color: #f3f4f6; font-size: 0.95rem; }
.import-instructions p { margin: 0.25rem 0; font-size: 0.85rem; color: #9ca3af; }
.import-instructions details { margin-top: 0.5rem; }
.import-instructions summary { cursor: pointer; color: var(--focus-ring); font-size: 0.85rem; }
.import-instructions pre { background: var(--bg-well); padding: 0.75rem; border-radius: 4px; overflow-x: auto; margin-top: 0.5rem; font-size: 0.8rem; }
.file-input { padding: 0.5rem; background: var(--bg-elevated); border: 1px dashed #4b5563; border-radius: 4px; cursor: pointer; }
.file-input::-webkit-file-upload-button { background: var(--border-input); color: white; border: none; padding: 0.5rem 1rem; border-radius: 4px; cursor: pointer; margin-right: 1rem; }
.import-result { background: var(--bg-elevated); border-radius: 6px; padding: 1rem; margin: 1rem 0; }
.import-result .success { color: var(--success); margin: 0.25rem 0; }
.import-result .error { color: var(--danger); margin: 0.25rem 0; }
.error-list { margin-top: 0.5rem; }
.error-list details { cursor: pointer; }
.error-list summary { color: #f97316; font-size: 0.9rem; }
.error-list ul { margin: 0.5rem 0 0 0; padding-left: 1.5rem; max-height: 200px; overflow-y: auto; }
.error-list li { color: #fca5a5; font-size: 0.85rem; margin: 0.25rem 0; }

@media (max-width: 900px) {
  .account-card { grid-template-columns: auto minmax(0, 1fr); }
  .actions { grid-column: 2; justify-content: flex-start; max-width: none; }
  .meta-grid { grid-template-columns: 1fr; }
}

@media (max-width: 768px) {
  .account-panel { padding: 1rem; }
  .panel-header { align-items: stretch; }
  .header-actions,
  .filter-input,
  .filter-select,
  .user-filter { width: 100%; }
  .header-actions > *,
  .filter-bar > * { width: 100%; }
  .filter-count,
  .selection-count { margin-left: 0; width: 100%; }
  .bulk-toolbar button,
  .actions button { flex: 1; }
  .account-card { padding: 0.9rem; }
  .batch-item { grid-template-columns: 1fr; align-items: start; }
  .account-list { grid-template-columns: 1fr; }
}
</style>
