<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'

type Role = 'ADMIN' | 'USER'
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
const isAdmin = computed(() => user.value?.role === 'ADMIN')
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
    throw new Error(data?.message || data?.error || `请求失败 (${response.status})`)
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
    notify('登录成功')
  } catch (error) {
    notify(error instanceof Error ? error.message : '登录失败')
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
    if (!data.user) throw new Error('登录已过期')
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
    notify('请填写账户名称和站点地址')
    return
  }

  actionLoading.value = 'save-account'
  try {
    const payload = accountPayload()
    if (editingAccountId.value) {
      await api(`/api/accounts/${editingAccountId.value}`, { method: 'PUT', body: JSON.stringify(payload) })
      notify('账户已更新')
    } else {
      await api('/api/accounts', { method: 'POST', body: JSON.stringify(payload) })
      notify('账户已新增')
    }
    resetAccountForm()
    await loadAccounts()
  } catch (error) {
    notify(error instanceof Error ? error.message : '保存失败')
  } finally {
    actionLoading.value = ''
  }
}

async function deleteAccount(account: Account) {
  if (!confirm(`确认删除账户「${account.name}」？`)) return
  actionLoading.value = account.id
  try {
    await api(`/api/accounts/${account.id}`, { method: 'DELETE' })
    await Promise.all([loadAccounts(), loadRuns()])
    notify('账户已删除')
  } catch (error) {
    notify(error instanceof Error ? error.message : '删除失败')
  } finally {
    actionLoading.value = ''
  }
}

async function executeCheckin(account: Account) {
  actionLoading.value = account.id
  try {
    await api('/api/checkin-runs', { method: 'POST', body: JSON.stringify({ account_id: account.id }) })
    await Promise.all([loadAccounts(), loadRuns()])
    notify('签到执行完成')
  } catch (error) {
    notify(error instanceof Error ? error.message : '签到失败')
  } finally {
    actionLoading.value = ''
  }
}

async function saveSettings() {
  actionLoading.value = 'settings'
  try {
    const data = await api<CheckinSetting>('/api/settings', {
      method: 'PUT',
      body: JSON.stringify(setting),
    })
    Object.assign(setting, data)
    notify('设置已保存')
  } catch (error) {
    notify(error instanceof Error ? error.message : '保存失败')
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
    notify('请填写用户名和密码')
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
      notify('用户已更新')
    } else {
      await api('/api/admin/users', { method: 'POST', body: JSON.stringify(payload) })
      notify('用户已创建')
    }
    resetUserForm()
    await loadUsers()
  } catch (error) {
    notify(error instanceof Error ? error.message : '保存失败')
  } finally {
    actionLoading.value = ''
  }
}

async function deleteUser(item: AppUser) {
  if (!confirm(`确认删除用户「${item.username}」？`)) return
  actionLoading.value = item.id
  try {
    await api(`/api/admin/users/${item.id}`, { method: 'DELETE' })
    await loadUsers()
    notify('用户已删除')
  } catch (error) {
    notify(error instanceof Error ? error.message : '删除失败')
  } finally {
    actionLoading.value = ''
  }
}

function formatDate(value?: string | null) {
  if (!value) return '未记录'
  return new Date(value).toLocaleString('zh-CN', { hour12: false })
}

function formatBalance(value?: number | null) {
  if (value === null || value === undefined) return '—'
  return value.toLocaleString('zh-CN', { maximumFractionDigits: 4 })
}

function statusText(status?: Status | null) {
  if (status === 'SUCCESS') return '成功'
  if (status === 'FAILED') return '失败'
  if (status === 'PENDING') return '进行中'
  return status || '未知'
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
        <h1>自动签到管理平台</h1>
        <p>管理 New API / AnyRouter 账户、定时窗口、运行记录和后台用户。</p>
        <div class="feature-grid">
          <span>账户托管</span>
          <span>手动签到</span>
          <span>失败重试</span>
          <span>权限管理</span>
        </div>
      </div>

      <form class="login-card" @submit.prevent="login">
        <h2>登录控制台</h2>
        <label>
          用户名
          <input v-model="loginForm.username" autocomplete="username" placeholder="admin" />
        </label>
        <label>
          密码
          <input v-model="loginForm.password" type="password" autocomplete="current-password" placeholder="admin123" />
        </label>
        <button class="primary" :disabled="loading">{{ loading ? '登录中...' : '登录' }}</button>
        <p v-if="message" class="message">{{ message }}</p>
      </form>
    </section>

    <template v-else>
      <header class="topbar">
        <div>
          <span class="eyebrow">Dashboard</span>
          <h1>签到控制台</h1>
        </div>
        <div class="userbar">
          <span>{{ user?.username }} · {{ user?.role }}</span>
          <button class="ghost" @click="loadAll">刷新</button>
          <button class="ghost danger" @click="logout">退出</button>
        </div>
      </header>

      <section class="stats">
        <article><span>账户总数</span><strong>{{ accounts.length }}</strong></article>
        <article><span>启用账户</span><strong>{{ enabledAccounts }}</strong></article>
        <article><span>成功记录</span><strong>{{ successRuns }}</strong></article>
        <article><span>总余额</span><strong>{{ formatBalance(totalBalance) }}</strong></article>
      </section>

      <p v-if="message" class="toast">{{ message }}</p>

      <nav class="tabs">
        <button :class="{ active: activeTab === 'accounts' }" @click="activeTab = 'accounts'">账户</button>
        <button :class="{ active: activeTab === 'runs' }" @click="activeTab = 'runs'">记录</button>
        <button :class="{ active: activeTab === 'settings' }" @click="activeTab = 'settings'">设置</button>
        <button v-if="isAdmin" :class="{ active: activeTab === 'users' }" @click="activeTab = 'users'">用户</button>
      </nav>

      <section v-if="activeTab === 'accounts'" class="layout">
        <form class="panel form" @submit.prevent="saveAccount">
          <div class="panel-title">
            <h2>{{ editingAccountId ? '编辑账户' : '新增账户' }}</h2>
            <button v-if="editingAccountId" type="button" class="ghost" @click="resetAccountForm">取消</button>
          </div>
          <label>名称<input v-model="accountForm.name" placeholder="主账号" /></label>
          <div class="two-cols">
            <label>站点类型
              <select v-model="accountForm.siteType">
                <option value="new-api">New API</option>
                <option value="anyrouter">AnyRouter</option>
                <option value="custom">自定义</option>
              </select>
            </label>
            <label>认证方式
              <select v-model="accountForm.authType">
                <option value="access-token">Access Token</option>
                <option value="cookie">Cookie</option>
              </select>
            </label>
          </div>
          <label>站点地址<input v-model="accountForm.baseUrl" placeholder="https://example.com" /></label>
          <label>用户 ID<input v-model="accountForm.userId" placeholder="可选" /></label>
          <label>Access Token<input v-model="accountForm.accessToken" :placeholder="editingAccountId ? '留空则不更新' : 'sk-...'" /></label>
          <label>Cookie<textarea v-model="accountForm.cookie" :placeholder="editingAccountId ? '留空则不更新' : 'session=...'" /></label>
          <label>自定义签到地址<input v-model="accountForm.customCheckinUrl" placeholder="可选" /></label>
          <div class="checks">
            <label><input v-model="accountForm.enabled" type="checkbox" /> 启用</label>
            <label><input v-model="accountForm.retryEnabled" type="checkbox" /> 失败重试</label>
          </div>
          <button class="primary" :disabled="actionLoading === 'save-account'">
            {{ actionLoading === 'save-account' ? '保存中...' : '保存账户' }}
          </button>
        </form>

        <div class="panel list-panel">
          <div class="panel-title"><h2>账户列表</h2><span>{{ accounts.length }} 个</span></div>
          <div v-if="accounts.length === 0" class="empty">暂无账户，先新增一个签到账户。</div>
          <article v-for="account in accounts" :key="account.id" class="account-card">
            <div class="card-head">
              <div>
                <h3>{{ account.name }}</h3>
                <p>{{ account.baseUrl }}</p>
              </div>
              <span class="badge" :class="account.enabled ? 'ok' : 'muted'">{{ account.enabled ? '启用' : '停用' }}</span>
            </div>
            <div class="meta">
              <span>{{ account.siteType }}</span>
              <span>{{ account.authType }}</span>
              <span>余额 {{ formatBalance(account.lastBalance) }}</span>
              <span :class="['status', account.lastStatus?.toLowerCase()]">{{ statusText(account.lastStatus) }}</span>
            </div>
            <p class="hint">{{ account.lastMessage || '暂无最近签到消息' }}</p>
            <div class="actions">
              <button class="primary small" :disabled="actionLoading === account.id" @click="executeCheckin(account)">
                {{ actionLoading === account.id ? '执行中...' : '签到' }}
              </button>
              <button class="ghost small" @click="editAccount(account)">编辑</button>
              <button class="ghost small danger" @click="deleteAccount(account)">删除</button>
            </div>
          </article>
        </div>
      </section>

      <section v-if="activeTab === 'runs'" class="panel">
        <div class="panel-title"><h2>签到记录</h2><button class="ghost" @click="loadRuns">刷新记录</button></div>
        <div class="table-wrap">
          <table>
            <thead><tr><th>账户</th><th>状态</th><th>触发</th><th>耗时</th><th>时间</th><th>消息</th></tr></thead>
            <tbody>
              <tr v-for="run in latestRuns" :key="run.id">
                <td>{{ accountName(run.accountId) }}</td>
                <td><span :class="['status', run.status.toLowerCase()]">{{ statusText(run.status) }}</span></td>
                <td>{{ run.triggeredBy }}</td>
                <td>{{ run.durationMs ?? '—' }} ms</td>
                <td>{{ formatDate(run.createdAt) }}</td>
                <td>{{ run.message || '—' }}</td>
              </tr>
            </tbody>
          </table>
        </div>
        <div v-if="runs.length === 0" class="empty">暂无签到记录。</div>
      </section>

      <section v-if="activeTab === 'settings'" class="panel form narrow">
        <div class="panel-title"><h2>全局设置</h2><span>定时签到策略</span></div>
        <div class="checks">
          <label><input v-model="setting.enabled" type="checkbox" /> 开启自动签到</label>
          <label><input v-model="setting.retryEnabled" type="checkbox" /> 开启失败重试</label>
        </div>
        <div class="two-cols">
          <label>开始时间<input v-model="setting.windowStart" type="time" /></label>
          <label>结束时间<input v-model="setting.windowEnd" type="time" /></label>
        </div>
        <label>每日最大尝试次数<input v-model.number="setting.maxAttemptsPerDay" min="1" type="number" /></label>
        <button class="primary" :disabled="actionLoading === 'settings'" @click="saveSettings">
          {{ actionLoading === 'settings' ? '保存中...' : '保存设置' }}
        </button>
      </section>

      <section v-if="activeTab === 'users' && isAdmin" class="layout">
        <form class="panel form" @submit.prevent="saveUser">
          <div class="panel-title">
            <h2>{{ editingUserId ? '编辑用户' : '新增用户' }}</h2>
            <button v-if="editingUserId" type="button" class="ghost" @click="resetUserForm">取消</button>
          </div>
          <label>用户名<input v-model="userForm.username" /></label>
          <label>密码<input v-model="userForm.password" type="password" :placeholder="editingUserId ? '留空则不更新' : ''" /></label>
          <label>角色
            <select v-model="userForm.role">
              <option value="USER">USER</option>
              <option value="ADMIN">ADMIN</option>
            </select>
          </label>
          <label>备注<input v-model="userForm.note" /></label>
          <label class="check-line"><input v-model="userForm.enabled" type="checkbox" /> 启用用户</label>
          <button class="primary" :disabled="actionLoading === 'save-user'">保存用户</button>
        </form>

        <div class="panel list-panel">
          <div class="panel-title"><h2>用户列表</h2><span>{{ users.length }} 个</span></div>
          <article v-for="item in users" :key="item.id" class="user-card">
            <div><h3>{{ item.username }}</h3><p>{{ item.note || '无备注' }}</p></div>
            <span class="badge">{{ item.role }}</span>
            <span class="badge" :class="item.enabled ? 'ok' : 'muted'">{{ item.enabled ? '启用' : '停用' }}</span>
            <button class="ghost small" @click="editUser(item)">编辑</button>
            <button class="ghost small danger" @click="deleteUser(item)">删除</button>
          </article>
        </div>
      </section>
    </template>
  </main>
</template>
