<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'

type Role = 'ADMIN' | 'USER' | 'SUPER_ADMIN'
type Status = 'SUCCESS' | 'FAILED' | 'PENDING' | string

interface AppUser {
  id: string
  username: string
  role: Role | string
  enabled: boolean
  note?: string | null
  createdAt?: string
  updatedAt?: string
}

interface Account {
  id: string
  name: string
  siteType: string
  baseUrl: string
  userId?: string | null
  authType: string
  accessTokenMasked?: string | null
  cookieMasked?: string | null
  customCheckinUrl?: string | null
  enabled: boolean
  retryEnabled: boolean
  lastBalance?: number | null
  lastBalanceAt?: string | null
  lastStatus?: Status | null
  lastMessage?: string | null
  lastRunAt?: string | null
  createdAt?: string
  updatedAt?: string
}

interface CheckinRun {
  id: string
  accountId: string
  status: Status
  message?: string | null
  durationMs?: number | null
  triggeredBy: string
  rawResponse?: string | null
  createdAt: string
}

interface CheckinSetting {
  id?: string
  enabled: boolean
  windowStart: string
  windowEnd: string
  retryEnabled: boolean
  maxAttemptsPerDay: number
  updatedAt?: string
}

interface AccountForm {
  name: string
  siteType: string
  baseUrl: string
  userId: string
  authType: string
  accessToken: string
  cookie: string
  customCheckinUrl: string
  enabled: boolean
  retryEnabled: boolean
}

const token = ref(localStorage.getItem('aihub_token') || '')
const user = ref<AppUser | null>(null)
const accounts = ref<Account[]>([])
const runs = ref<CheckinRun[]>([])
const users = ref<AppUser[]>([])
const loading = ref(false)
const actionLoading = ref('')
const message = ref('')
const activeTab = ref<'accounts' | 'runs' | 'settings' | 'users'>('accounts')
const editingAccountId = ref('')
const editingUserId = ref('')
const filterUserId = ref<string>('')

const loginForm = reactive({ username: 'admin', password: 'admin123' })
const accountForm = reactive<AccountForm>(emptyAccountForm())
const setting = reactive<CheckinSetting>({
  enabled: true,
  windowStart: '08:00',
  windowEnd: '10:00',
  retryEnabled: true,
  maxAttemptsPerDay: 3,
})
const userForm = reactive({ username: '', password: '', role: 'USER', enabled: true, note: '' })

const isAuthed = computed(() => Boolean(token.value && user.value))
const isAdmin = computed(() => user.value?.role === 'ADMIN' || user.value?.role === 'SUPER_ADMIN')
const enabledAccounts = computed(() => accounts.value.filter((account) => account.enabled).length)
const successRuns = computed(() => runs.value.filter((run) => run.status === 'SUCCESS').length)
const latestRuns = computed(() => runs.value.slice(0, 8))
const totalBalance = computed(() => accounts.value.reduce((sum, account) => sum + (account.lastBalance || 0), 0))

function emptyAccountForm(): AccountForm {
  return {
    name: '',
    siteType: 'new-api',
    baseUrl: '',
    userId: '',
    authType: 'access-token',
    accessToken: '',
    cookie: '',
    customCheckinUrl: '',
    enabled: true,
    retryEnabled: true,
  }
}

async function api<T>(path: string, options: RequestInit = {}): Promise<T> {
  const headers = new Headers(options.headers)
  headers.set('Content-Type', 'application/json')
  if (token.value) headers.set('Authorization', `Bearer ${token.value}`)

  const response = await fetch(path, { ...options, headers })
  const text = await response.text()
  const data = text ? JSON.parse(text) : null

  if (!response.ok) {
    throw new Error(data?.message || data?.error || `璇锋眰澶辫触 (${response.status})`)
  }

  return data as T
}

function notify(text: string) {
  message.value = text
  window.setTimeout(() => {
    if (message.value === text) message.value = ''
  }, 3500)
}

async function login() {
  loading.value = true
  try {
    const data = await api<{ token: string; user: AppUser }>('/api/auth/login', {
      method: 'POST',
      body: JSON.stringify(loginForm),
    })
    token.value = data.token
    user.value = data.user
    localStorage.setItem('aihub_token', data.token)
    await loadAll()
    notify('鐧诲綍鎴愬姛')
  } catch (error) {
    notify(error instanceof Error ? error.message : '鐧诲綍澶辫触')
  } finally {
    loading.value = false
  }
}

async function logout() {
  try {
    await api('/api/auth/logout', { method: 'POST' })
  } catch {
    // ignore logout errors and clear local session
  }
  token.value = ''
  user.value = null
  localStorage.removeItem('aihub_token')
}

async function restoreSession() {
  if (!token.value) return
  try {
    const data = await api<{ user: AppUser | null }>('/api/auth/me')
    if (!data.user) throw new Error('鐧诲綍宸茶繃鏈?)
    user.value = data.user
    await loadAll()
  } catch {
    await logout()
  }
}

async function loadAll() {
  loading.value = true
  try {
    await Promise.all([loadAccounts(), loadRuns(), loadSettings(), isAdmin.value ? loadUsers() : Promise.resolve()])
  } finally {
    loading.value = false
  }
}

async function loadAccounts() {
  accounts.value = await api<Account[]>('/api/accounts')
}

async function loadRuns() {
  runs.value = await api<CheckinRun[]>('/api/checkin-runs')
}

async function loadSettings() {
  const data = await api<CheckinSetting>('/api/settings')
  Object.assign(setting, data)
}

async function loadUsers() {
  if (!isAdmin.value) return
  users.value = await api<AppUser[]>('/api/admin/users')
}

function editAccount(account: Account) {
  editingAccountId.value = account.id
  Object.assign(accountForm, {
    name: account.name,
    siteType: account.siteType,
    baseUrl: account.baseUrl,
    userId: account.userId || '',
    authType: account.authType,
    accessToken: '',
    cookie: '',
    customCheckinUrl: account.customCheckinUrl || '',
    enabled: account.enabled,
    retryEnabled: account.retryEnabled,
  })
  window.scrollTo({ top: 0, behavior: 'smooth' })
}

function resetAccountForm() {
  editingAccountId.value = ''
  Object.assign(accountForm, emptyAccountForm())
}

function accountPayload() {
  return {
    name: accountForm.name.trim(),
    siteType: accountForm.siteType,
    baseUrl: accountForm.baseUrl.trim(),
    userId: accountForm.userId.trim() || null,
    authType: accountForm.authType,
    accessToken: accountForm.accessToken.trim() || null,
    cookie: accountForm.cookie.trim() || null,
    customCheckinUrl: accountForm.customCheckinUrl.trim() || null,
    enabled: accountForm.enabled,
    retryEnabled: accountForm.retryEnabled,
  }
}

async function saveAccount() {
  if (!accountForm.name || !accountForm.baseUrl) {
    notify('璇峰～鍐欒处鎴峰悕绉板拰绔欑偣鍦板潃')
    return
  }

  actionLoading.value = 'save-account'
  try {
    const payload = accountPayload()
    if (editingAccountId.value) {
      await api(`/api/accounts/${editingAccountId.value}`, { method: 'PUT', body: JSON.stringify(payload) })
      notify('璐︽埛宸叉洿鏂?)
    } else {
      await api('/api/accounts', { method: 'POST', body: JSON.stringify(payload) })
      notify('璐︽埛宸叉柊澧?)
    }
    resetAccountForm()
    await loadAccounts()
  } catch (error) {
    notify(error instanceof Error ? error.message : '淇濆瓨澶辫触')
  } finally {
    actionLoading.value = ''
  }
}

async function deleteAccount(account: Account) {
  if (!confirm(`纭鍒犻櫎璐︽埛銆?{account.name}銆嶏紵`)) return
  actionLoading.value = account.id
  try {
    await api(`/api/accounts/${account.id}`, { method: 'DELETE' })
    await Promise.all([loadAccounts(), loadRuns()])
    notify('璐︽埛宸插垹闄?)
  } catch (error) {
    notify(error instanceof Error ? error.message : '鍒犻櫎澶辫触')
  } finally {
    actionLoading.value = ''
  }
}

async function executeCheckin(account: Account) {
  actionLoading.value = account.id
  try {
    await api('/api/checkin-runs', { method: 'POST', body: JSON.stringify({ account_id: account.id }) })
    await Promise.all([loadAccounts(), loadRuns()])
    notify('绛惧埌鎵ц瀹屾垚')
  } catch (error) {
    notify(error instanceof Error ? error.message : '绛惧埌澶辫触')
  } finally {
    actionLoading.value = ''
  }
}


async function refreshBalance(account: Account) {
  actionLoading.value = account.id
  try {
    await api(`/api/accounts/${account.id}/refresh-balance`, { method: ''POST'' })
    await loadAccounts()
    notify(''余额已刷新'')
  } catch (error) {
    notify(error instanceof Error ? error.message : ''余额刷新失败'')
  } finally {
    actionLoading.value = ''''
  }
}

async function cleanupRuns() {
  if (!confirm(''确认清理签到记录？将只保留最近 100 条'')) return
  actionLoading.value = ''cleanup''
  try {
    const response = await api(''/api/checkin-runs/cleanup'', {
      method: ''DELETE'',
      body: JSON.stringify({ keepLatest: 100 })
    })
    await loadRuns()
    notify(`已清理 ${response.deletedCount} 条记录`)
  } catch (error) {
    notify(error instanceof Error ? error.message : ''清理失败'')
  } finally {
    actionLoading.value = ''''
  }
}async function saveSettings() {
  actionLoading.value = 'settings'
  try {
    const data = await api<CheckinSetting>('/api/settings', {
      method: 'PUT',
      body: JSON.stringify(setting),
    })
    Object.assign(setting, data)
    notify('璁剧疆宸蹭繚瀛?)
  } catch (error) {
    notify(error instanceof Error ? error.message : '淇濆瓨澶辫触')
  } finally {
    actionLoading.value = ''
  }
}

function editUser(item: AppUser) {
  editingUserId.value = item.id
  Object.assign(userForm, {
    username: item.username,
    password: '',
    role: item.role,
    enabled: item.enabled,
    note: item.note || '',
  })
}

function resetUserForm() {
  editingUserId.value = ''
  Object.assign(userForm, { username: '', password: '', role: 'USER', enabled: true, note: '' })
}

async function saveUser() {
  if (!userForm.username || (!editingUserId.value && !userForm.password)) {
    notify('璇峰～鍐欑敤鎴峰悕鍜屽瘑鐮?)
    return
  }

  actionLoading.value = 'save-user'
  try {
    const payload = {
      username: userForm.username.trim(),
      password: userForm.password || undefined,
      role: userForm.role,
      enabled: userForm.enabled,
      note: userForm.note || null,
    }
    if (editingUserId.value) {
      await api(`/api/admin/users/${editingUserId.value}`, { method: 'PUT', body: JSON.stringify(payload) })
      notify('鐢ㄦ埛宸叉洿鏂?)
    } else {
      await api('/api/admin/users', { method: 'POST', body: JSON.stringify(payload) })
      notify('鐢ㄦ埛宸插垱寤?)
    }
    resetUserForm()
    await loadUsers()
  } catch (error) {
    notify(error instanceof Error ? error.message : '淇濆瓨澶辫触')
  } finally {
    actionLoading.value = ''
  }
}

async function deleteUser(item: AppUser) {
  if (!confirm(`纭鍒犻櫎鐢ㄦ埛銆?{item.username}銆嶏紵`)) return
  actionLoading.value = item.id
  try {
    await api(`/api/admin/users/${item.id}`, { method: 'DELETE' })
    await loadUsers()
    notify('鐢ㄦ埛宸插垹闄?)
  } catch (error) {
    notify(error instanceof Error ? error.message : '鍒犻櫎澶辫触')
  } finally {
    actionLoading.value = ''
  }
}

function formatDate(value?: string | null) {
  if (!value) return '鏈褰?
  return new Date(value).toLocaleString('zh-CN', { hour12: false })
}

function formatBalance(value?: number | null) {
  if (value === null || value === undefined) return '鈥?
  return value.toLocaleString('zh-CN', { maximumFractionDigits: 4 })
}

function statusText(status?: Status | null) {
  if (status === 'SUCCESS') return '鎴愬姛'
  if (status === 'FAILED') return '澶辫触'
  if (status === 'PENDING') return '杩涜涓?
  return status || '鏈煡'
}

function accountName(accountId: string) {
  return accounts.value.find((account) => account.id === accountId)?.name || accountId.slice(0, 8)
}

onMounted(restoreSession)
</script>

<template>
  <main class="shell">
    <section v-if="!isAuthed" class="login-page">
      <div class="brand-card">
        <span class="eyebrow">AI Hub Rust</span>
        <h1>鑷姩绛惧埌绠＄悊骞冲彴</h1>
        <p>绠＄悊 New API / AnyRouter 璐︽埛銆佸畾鏃剁獥鍙ｃ€佽繍琛岃褰曞拰鍚庡彴鐢ㄦ埛銆?/p>
        <div class="feature-grid">
          <span>璐︽埛鎵樼</span>
          <span>鎵嬪姩绛惧埌</span>
          <span>澶辫触閲嶈瘯</span>
          <span>鏉冮檺绠＄悊</span>
        </div>
      </div>

      <form class="login-card" @submit.prevent="login">
        <h2>鐧诲綍鎺у埗鍙?/h2>
        <label>
          鐢ㄦ埛鍚?
          <input v-model="loginForm.username" autocomplete="username" placeholder="admin" />
        </label>
        <label>
          瀵嗙爜
          <input v-model="loginForm.password" type="password" autocomplete="current-password" placeholder="admin123" />
        </label>
        <button class="primary" :disabled="loading">{{ loading ? '鐧诲綍涓?..' : '鐧诲綍' }}</button>
        <p v-if="message" class="message">{{ message }}</p>
      </form>
    </section>

    <template v-else>
      <header class="topbar">
        <div>
          <span class="eyebrow">Dashboard</span>
          <h1>绛惧埌鎺у埗鍙?/h1>
        </div>
        <div class="userbar">
          <span>{{ user?.username }} 路 {{ user?.role }}</span>
          <button class="ghost" @click="loadAll">鍒锋柊</button>
          <button class="ghost danger" @click="logout">閫€鍑?/button>
        </div>
      </header>

      <section class="stats">
        <article><span>璐︽埛鎬绘暟</span><strong>{{ accounts.length }}</strong></article>
        <article><span>鍚敤璐︽埛</span><strong>{{ enabledAccounts }}</strong></article>
        <article><span>鎴愬姛璁板綍</span><strong>{{ successRuns }}</strong></article>
        <article><span>鎬讳綑棰?/span><strong>{{ formatBalance(totalBalance) }}</strong></article>
      </section>

      <p v-if="message" class="toast">{{ message }}</p>

      <nav class="tabs">
        <button :class="{ active: activeTab === 'accounts' }" @click="activeTab = 'accounts'">璐︽埛</button>
        <button :class="{ active: activeTab === 'runs' }" @click="activeTab = 'runs'">璁板綍</button>
        <button :class="{ active: activeTab === 'settings' }" @click="activeTab = 'settings'">璁剧疆</button>
        <button v-if="isAdmin" :class="{ active: activeTab === 'users' }" @click="activeTab = 'users'">鐢ㄦ埛</button>
      </nav>

      <section v-if="activeTab === 'accounts'" class="layout">
        <form class="panel form" @submit.prevent="saveAccount">
          <div class="panel-title">
            <h2>{{ editingAccountId ? '缂栬緫璐︽埛' : '鏂板璐︽埛' }}</h2>
            <button v-if="editingAccountId" type="button" class="ghost" @click="resetAccountForm">鍙栨秷</button>
          </div>
          <label>鍚嶇О<input v-model="accountForm.name" placeholder="涓昏处鍙? /></label>
          <div class="two-cols">
            <label>绔欑偣绫诲瀷
              <select v-model="accountForm.siteType">
                <option value="new-api">New API</option>
                <option value="anyrouter">AnyRouter</option>
                <option value="custom">鑷畾涔?/option>
              </select>
            </label>
            <label>璁よ瘉鏂瑰紡
              <select v-model="accountForm.authType">
                <option value="access-token">Access Token</option>
                <option value="cookie">Cookie</option>
              </select>
            </label>
          </div>
          <label>绔欑偣鍦板潃<input v-model="accountForm.baseUrl" placeholder="https://example.com" /></label>
          <label>鐢ㄦ埛 ID<input v-model="accountForm.userId" placeholder="鍙€? /></label>
          <label>Access Token<input v-model="accountForm.accessToken" :placeholder="editingAccountId ? '鐣欑┖鍒欎笉鏇存柊' : 'sk-...'" /></label>
          <label>Cookie<textarea v-model="accountForm.cookie" :placeholder="editingAccountId ? '鐣欑┖鍒欎笉鏇存柊' : 'session=...'" /></label>
          <label>鑷畾涔夌鍒板湴鍧€<input v-model="accountForm.customCheckinUrl" placeholder="鍙€? /></label>
          <div class="checks">
            <label><input v-model="accountForm.enabled" type="checkbox" /> 鍚敤</label>
            <label><input v-model="accountForm.retryEnabled" type="checkbox" /> 澶辫触閲嶈瘯</label>
          </div>
          <button class="primary" :disabled="actionLoading === 'save-account'">
            {{ actionLoading === 'save-account' ? '淇濆瓨涓?..' : '淇濆瓨璐︽埛' }}
          </button>
        </form>

        <div class="panel list-panel">
          <div class="panel-title"><h2>账户列表</h2><select v-if="isAdmin" v-model="filterUserId" @change="loadAccounts" style="margin: 0 8px; padding: 4px 8px;"><option value="">所有用户</option><option :value="user?.id">仅我的</option><option v-for="u in users" :key="u.id" :value="u.id" v-if="u.id !== user?.id">{{ u.username }}</option></select><span>{{ accounts.length }} 个</span></div>
          <div v-if="accounts.length === 0" class="empty">鏆傛棤璐︽埛锛屽厛鏂板涓€涓鍒拌处鎴枫€?/div>
          <article v-for="account in accounts" :key="account.id" class="account-card">
            <div class="card-head">
              <div>
                <h3>{{ account.name }}</h3>
                <p>{{ account.baseUrl }}</p>
              </div>
              <span class="badge" :class="account.enabled ? 'ok' : 'muted'">{{ account.enabled ? '鍚敤' : '鍋滅敤' }}</span>
            </div>
            <div class="meta">
              <span>{{ account.siteType }}</span>
              <span>{{ account.authType }}</span>
              <span>浣欓 {{ formatBalance(account.lastBalance) }}</span>
              <span :class="['status', account.lastStatus?.toLowerCase()]">{{ statusText(account.lastStatus) }}</span>
            </div>
            <p class="hint">{{ account.lastMessage || '鏆傛棤鏈€杩戠鍒版秷鎭? }}</p>
            <div class="actions">
              <button class="primary small" :disabled="actionLoading === account.id" @click="executeCheckin(account)">
              <button class="ghost small" :disabled="actionLoading === account.id" @click="refreshBalance(account)">
                {{ actionLoading === account.id ? '查询中...' : '查询余额' }}
              </button>
                {{ actionLoading === account.id ? '鎵ц涓?..' : '绛惧埌' }}
              </button>
              <button class="ghost small" @click="editAccount(account)">缂栬緫</button>
              <button class="ghost small danger" @click="deleteAccount(account)">鍒犻櫎</button>
            </div>
          </article>
        </div>
      </section>

      <section v-if="activeTab === 'runs'" class="panel">
        <div class="panel-title"><h2>签到记录</h2><button class="ghost" @click="loadRuns">刷新记录</button><button v-if="isAdmin" class="ghost danger" :disabled="actionLoading === 'cleanup'" @click="cleanupRuns">清理记录</button></div>
        <div class="table-wrap">
          <table>
            <thead><tr><th>璐︽埛</th><th>鐘舵€?/th><th>瑙﹀彂</th><th>鑰楁椂</th><th>鏃堕棿</th><th>娑堟伅</th></tr></thead>
            <tbody>
              <tr v-for="run in latestRuns" :key="run.id">
                <td>{{ accountName(run.accountId) }}</td>
                <td><span :class="['status', run.status.toLowerCase()]">{{ statusText(run.status) }}</span></td>
                <td>{{ run.triggeredBy }}</td>
                <td>{{ run.durationMs ?? '鈥? }} ms</td>
                <td>{{ formatDate(run.createdAt) }}</td>
                <td>{{ run.message || '鈥? }}</td>
              </tr>
            </tbody>
          </table>
        </div>
        <div v-if="runs.length === 0" class="empty">鏆傛棤绛惧埌璁板綍銆?/div>
      </section>

      <section v-if="activeTab === 'settings'" class="panel form narrow">
        <div class="panel-title"><h2>鍏ㄥ眬璁剧疆</h2><span>瀹氭椂绛惧埌绛栫暐</span></div>
        <div class="checks">
          <label><input v-model="setting.enabled" type="checkbox" /> 寮€鍚嚜鍔ㄧ鍒?/label>
          <label><input v-model="setting.retryEnabled" type="checkbox" /> 寮€鍚け璐ラ噸璇?/label>
        </div>
        <div class="two-cols">
          <label>寮€濮嬫椂闂?input v-model="setting.windowStart" type="time" /></label>
          <label>缁撴潫鏃堕棿<input v-model="setting.windowEnd" type="time" /></label>
        </div>
        <label>姣忔棩鏈€澶у皾璇曟鏁?input v-model.number="setting.maxAttemptsPerDay" min="1" type="number" /></label>
        <button class="primary" :disabled="actionLoading === 'settings'" @click="saveSettings">
          {{ actionLoading === 'settings' ? '淇濆瓨涓?..' : '淇濆瓨璁剧疆' }}
        </button>
      </section>

      <section v-if="activeTab === 'users' && isAdmin" class="layout">
        <form class="panel form" @submit.prevent="saveUser">
          <div class="panel-title">
            <h2>{{ editingUserId ? '缂栬緫鐢ㄦ埛' : '鏂板鐢ㄦ埛' }}</h2>
            <button v-if="editingUserId" type="button" class="ghost" @click="resetUserForm">鍙栨秷</button>
          </div>
          <label>鐢ㄦ埛鍚?input v-model="userForm.username" /></label>
          <label>瀵嗙爜<input v-model="userForm.password" type="password" :placeholder="editingUserId ? '鐣欑┖鍒欎笉鏇存柊' : ''" /></label>
          <label>瑙掕壊
            <select v-model="userForm.role">
              <option value="USER">USER</option>
              <option value="ADMIN">ADMIN</option>
            </select>
          </label>
          <label>澶囨敞<input v-model="userForm.note" /></label>
          <label class="check-line"><input v-model="userForm.enabled" type="checkbox" /> 鍚敤鐢ㄦ埛</label>
          <button class="primary" :disabled="actionLoading === 'save-user'">淇濆瓨鐢ㄦ埛</button>
        </form>

        <div class="panel list-panel">
          <div class="panel-title"><h2>鐢ㄦ埛鍒楄〃</h2><span>{{ users.length }} 涓?/span></div>
          <article v-for="item in users" :key="item.id" class="user-card">
            <div><h3>{{ item.username }}</h3><p>{{ item.note || '鏃犲娉? }}</p></div>
            <span class="badge">{{ item.role }}</span>
            <span class="badge" :class="item.enabled ? 'ok' : 'muted'">{{ item.enabled ? '鍚敤' : '鍋滅敤' }}</span>
            <button class="ghost small" @click="editUser(item)">缂栬緫</button>
            <button class="ghost small danger" @click="deleteUser(item)">鍒犻櫎</button>
          </article>
        </div>
      </section>
    </template>
  </main>
</template>
