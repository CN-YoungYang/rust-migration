<template>
  <div id="app">
    <!-- 离线提示条 -->
    <div v-if="!isOnline" class="offline-banner" role="status" aria-live="polite">
      网络连接已断开，请检查网络设置
    </div>

    <nav v-if="isLoggedIn" class="navbar" aria-label="主导航">
      <div class="brand">
        <h1>AI Hub</h1>
        <span class="user-chip">{{ currentUser?.username }} · {{ roleText }}</span>
      </div>
      <div class="nav-links" aria-label="功能导航">
        <button @click="selectView('accounts')" :class="{ active: currentView === 'accounts' }" :aria-current="currentView === 'accounts' ? 'page' : undefined">账户管理</button>
        <button @click="selectView('runs')" :class="{ active: currentView === 'runs' }" :aria-current="currentView === 'runs' ? 'page' : undefined">签到记录</button>
        <button @click="selectView('statistics')" :class="{ active: currentView === 'statistics' }" :aria-current="currentView === 'statistics' ? 'page' : undefined">数据统计</button>
        <button @click="selectView('notifications')" :class="{ active: currentView === 'notifications' }" :aria-current="currentView === 'notifications' ? 'page' : undefined">通知设置</button>
        <button @click="selectView('settings')" :class="{ active: currentView === 'settings' }" v-if="isAdmin" :aria-current="currentView === 'settings' ? 'page' : undefined">全局设置</button>
        <button @click="selectView('users')" :class="{ active: currentView === 'users' }" v-if="isAdmin" :aria-current="currentView === 'users' ? 'page' : undefined">用户管理</button>
        <button @click="logout" class="btn-logout">退出</button>
      </div>
      <div
        class="server-status"
        :title="serverTime || '服务器时间'"
        @mouseenter="startHoverTimer"
        @mouseleave="stopHoverTimer"
        @focus="startHoverTimer"
        @blur="stopHoverTimer"
        role="status"
        tabindex="0"
        :aria-label="serverStatusLabel"
      >
        <span class="status-dot" :class="serverStatusClass" :aria-hidden="true"></span>
        <span class="status-text">{{ serverStatusText }}</span>
      </div>
    </nav>

    <main class="container">
      <div v-if="authChecking" class="login-page">
        <div class="loading-panel" role="status" aria-live="polite" aria-busy="true">正在检查登录状态...</div>
      </div>

      <div v-else-if="!isLoggedIn" class="login-page">
        <form @submit.prevent="login" class="login-form" aria-labelledby="login-title">
          <h2 id="login-title">登录</h2>
          <div class="form-group">
            <label class="sr-only" for="login-username">用户名</label>
            <input id="login-username" v-model="loginForm.username" name="username" placeholder="用户名" autocomplete="username" autocapitalize="none" required :disabled="loginLoading" />
          </div>
          <div class="form-group">
            <label class="sr-only" for="login-password">密码</label>
            <input id="login-password" v-model="loginForm.password" name="password" type="password" placeholder="密码" autocomplete="current-password" required :disabled="loginLoading" />
          </div>
          <button type="submit" class="btn-primary" :disabled="loginLoading">
            {{ loginLoading ? '登录中...' : '登录' }}
          </button>
          <p v-if="error" class="error" role="alert" aria-live="assertive">{{ error }}</p>
        </form>
      </div>

      <div
        v-else
        ref="panelRegion"
        class="panel-region"
        role="region"
        :aria-label="`${currentViewLabel}面板`"
        tabindex="-1"
      >
        <KeepAlive :include="cachedPanelNames">
          <component :is="activePanelComponent" v-bind="activePanelProps" />
        </KeepAlive>
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, nextTick, onMounted, onUnmounted, type Component } from 'vue'
import AccountPanel from './components/AccountPanel.vue'
import CheckinRunsPanel from './components/CheckinRunsPanel.vue'
import StatisticsPanel from './components/StatisticsPanel.vue'
import NotificationPanel from './components/NotificationPanel.vue'
import SettingsPanel from './components/SettingsPanel.vue'
import AdminUserPanel from './components/AdminUserPanel.vue'
import { AUTH_EXPIRED_EVENT, apiUrl, request, responseData } from './utils/api'
import { showToast } from './utils/toast'
interface AppUser {
  id: string
  username: string
  role: string
  enabled: boolean
}

type ViewName = 'accounts' | 'runs' | 'statistics' | 'notifications' | 'settings' | 'users'

const panelComponents: Record<ViewName, Component> = {
  accounts: AccountPanel,
  runs: CheckinRunsPanel,
  statistics: StatisticsPanel,
  notifications: NotificationPanel,
  settings: SettingsPanel,
  users: AdminUserPanel,
}

const viewLabels: Record<ViewName, string> = {
  accounts: '账户管理',
  runs: '签到记录',
  statistics: '数据统计',
  notifications: '通知设置',
  settings: '全局设置',
  users: '用户管理',
}

const cachedPanelNames = ['AccountPanel', 'CheckinRunsPanel', 'StatisticsPanel']

const isLoggedIn = ref(false)
const currentUser = ref<AppUser | null>(null)
const currentView = ref<ViewName>('accounts')
const loginForm = ref({ username: '', password: '' })
const error = ref('')
const authChecking = ref(true)
const loginLoading = ref(false)
const serverOk = ref<boolean | null>(null)
const serverTime = ref('')
const isOnline = ref(navigator.onLine)
const panelRegion = ref<HTMLElement | null>(null)
let serverTimeOffset = 0 // 服务器时间与本地时间的差值（毫秒）
let hoverTimer: ReturnType<typeof setInterval> | null = null
let serverTimeSyncTimer: ReturnType<typeof setInterval> | null = null

const isAdmin = computed(() => {
  return currentUser.value?.role === 'ADMIN' || currentUser.value?.role === 'SUPER_ADMIN'
})

const roleText = computed(() => {
  const map: Record<string, string> = {
    USER: '普通用户',
    ADMIN: '管理员',
    SUPER_ADMIN: '超级管理员'
  }
  return map[currentUser.value?.role || ''] || '用户'
})

const currentViewLabel = computed(() => viewLabels[currentView.value])
const activePanelComponent = computed(() => panelComponents[currentView.value])
const activePanelProps = computed<Record<string, unknown>>(() => {
  if (currentView.value === 'users') return { currentUser: currentUser.value }
  if (currentView.value === 'notifications' || currentView.value === 'settings') return {}
  return { currentUser: currentUser.value, isAdmin: isAdmin.value }
})
const serverStatusText = computed(() => {
  if (serverOk.value === null) return '检测中'
  return serverOk.value ? '在线' : '离线'
})
const serverStatusClass = computed(() => {
  if (serverOk.value === null) return 'checking'
  return serverOk.value ? 'online' : 'offline'
})
const serverStatusLabel = computed(() => {
  const time = serverTime.value ? `，服务器时间 ${serverTime.value}` : ''
  return `服务器${serverStatusText.value}${time}`
})

const selectView = (view: ViewName) => {
  if (currentView.value === view) return
  currentView.value = view
  void nextTick(() => panelRegion.value?.focus())
}

const login = async () => {
  if (loginLoading.value) return
  error.value = ''
  loginLoading.value = true
  try {
    const res = await request(apiUrl('/auth/login'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(loginForm.value)
    })
    const data = await responseData<{ user: AppUser | null }>(res)
    if (!data.user) throw new Error('登录响应异常')
    currentUser.value = data.user
    isLoggedIn.value = true
    authChecking.value = false
  } catch (e) {
    error.value = e instanceof Error ? e.message : '登录失败'
  } finally {
    loginLoading.value = false
  }
}

const fetchCurrentUser = async () => {
  try {
    const res = await request(apiUrl('/auth/me'))
    const data = await responseData<{ user: AppUser | null }>(res)
    currentUser.value = data.user
    isLoggedIn.value = !!data.user
  } catch {
    isLoggedIn.value = false
    currentUser.value = null
  } finally {
    authChecking.value = false
  }
}

const clearSessionState = () => {
  isLoggedIn.value = false
  currentUser.value = null
  currentView.value = 'accounts'
}

const logout = async () => {
  try {
    await request(apiUrl('/auth/logout'), { method: 'POST' })
  } catch {
    // 本地退出优先，不阻塞用户操作
  }
  clearSessionState()
}

let healthTimer: ReturnType<typeof setInterval> | null = null

const checkHealth = async () => {
  try {
    const res = await fetch(apiUrl('/health'), { signal: AbortSignal.timeout(5000) })
    serverOk.value = res.ok
  } catch {
    serverOk.value = false
  }
}

const updateDisplayTime = () => {
  if (serverTimeOffset === 0) return
  const now = new Date(Date.now() + serverTimeOffset)
  serverTime.value = now.toLocaleString('zh-CN')
}

const fetchServerTime = async () => {
  try {
    const localBefore = Date.now()
    const res = await fetch(apiUrl('/server-time'), { signal: AbortSignal.timeout(5000) })
    if (res.ok) {
      const data = await responseData<{ serverTime: string }>(res)
      const serverMs = new Date(data.serverTime).getTime()
      const localAfter = Date.now()
      // 补偿网络延迟：取请求前后本地时间的中点
      const localMid = (localBefore + localAfter) / 2
      serverTimeOffset = serverMs - localMid
      updateDisplayTime()
    }
  } catch {
    // 静默失败
  }
}

const startHoverTimer = () => {
  // 鼠标悬停时启动 1 秒定时器，流畅显示服务器时间
  if (hoverTimer) return
  updateDisplayTime() // 立即更新一次
  hoverTimer = setInterval(updateDisplayTime, 1000)
}

const stopHoverTimer = () => {
  // 鼠标移开时停止定时器
  if (hoverTimer) {
    clearInterval(hoverTimer)
    hoverTimer = null
  }
}

const handleOnline = () => {
  isOnline.value = true
  checkHealth()
}

const handleOffline = () => {
  isOnline.value = false
}

const handleAuthExpired = () => {
  if (!isLoggedIn.value) return
  clearSessionState()
  error.value = '登录已过期，请重新登录'
  showToast('登录已过期，请重新登录', 'error')
}

onMounted(() => {
  fetchCurrentUser()
  checkHealth()
  fetchServerTime()
  // 健康检查：每 5 分钟（降低频率，减少不必要的请求）
  healthTimer = setInterval(checkHealth, 300000)
  // 每 5 分钟重新同步一次服务器时间，防止本地时钟漂移
  serverTimeSyncTimer = setInterval(fetchServerTime, 300000)

  // 离线检测
  window.addEventListener('online', handleOnline)
  window.addEventListener('offline', handleOffline)
  window.addEventListener(AUTH_EXPIRED_EVENT, handleAuthExpired)
})

onUnmounted(() => {
  if (healthTimer) clearInterval(healthTimer)
  if (hoverTimer) clearInterval(hoverTimer)
  if (serverTimeSyncTimer) clearInterval(serverTimeSyncTimer)
  window.removeEventListener('online', handleOnline)
  window.removeEventListener('offline', handleOffline)
  window.removeEventListener(AUTH_EXPIRED_EVENT, handleAuthExpired)
})
</script>

<style>
#app { min-height: 100vh; background: radial-gradient(circle at top left, rgba(16, 185, 129, 0.08), transparent 34rem), #0b0d10; }
.offline-banner {
  background: var(--warn);
  color: #000;
  text-align: center;
  padding: 0.75rem;
  font-weight: 500;
  position: sticky;
  top: 0;
  z-index: 1000;
  box-shadow: 0 2px 4px rgba(0,0,0,0.2);
}
.navbar { background: rgba(17, 24, 39, 0.92); padding: 0.85rem 2rem; display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid var(--border); position: sticky; top: 0; z-index: 30; backdrop-filter: blur(10px); gap: 1rem; }
.brand { display: flex; align-items: center; gap: 0.75rem; min-width: 0; }
.navbar h1 { font-size: 1.35rem; letter-spacing: 0; color: var(--text-strong); white-space: nowrap; }
.user-chip { color: var(--text-faint); background: var(--bg-app); border: 1px solid var(--border); border-radius: var(--radius-pill); padding: 0.25rem 0.55rem; font-size: 0.78rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 220px; }
.nav-links { display: flex; gap: 0.4rem; align-items: center; }
.nav-links button { background: transparent; color: var(--text-faint); border: 1px solid transparent; padding: 0.48rem 0.8rem; cursor: pointer; border-radius: 6px; transition: all 0.16s ease; }
.nav-links button.active { background: var(--accent); border-color: var(--accent-border); color: white; }
.nav-links button:hover:not(.active) { background: var(--bg-elevated); border-color: var(--border-strong); color: var(--text-strong); }
.btn-logout { background: #b91c1c; color: white; border: none; padding: 0.5rem 1rem; border-radius: 6px; cursor: pointer; }
.btn-logout:hover { background: #dc2626; }
.container { max-width: 1400px; margin: 0 auto; padding: 1.5rem; }
.login-page { display: flex; align-items: center; justify-content: center; min-height: 80vh; }
.loading-panel { background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius); color: var(--text-faint); padding: 1.2rem 1.5rem; }
.login-form { background: var(--bg-card); border: 1px solid var(--border); padding: 2rem; border-radius: var(--radius); width: 100%; max-width: 400px; box-shadow: var(--shadow-modal); }
.login-form h2 { margin-bottom: 1.5rem; text-align: center; color: var(--text-strong); }
.form-group { margin-bottom: 1rem; }
.form-group input { width: 100%; padding: 0.75rem; background: var(--bg-well); border: 1px solid var(--border-strong); border-radius: 6px; color: var(--text-strong); font-size: 1rem; }
.btn-primary { width: 100%; background: var(--accent); color: white; border: none; padding: 0.75rem; border-radius: 6px; cursor: pointer; font-size: 1rem; font-weight: 600; }
.btn-primary:hover:not(:disabled) { background: var(--accent-hover); }
.btn-primary:disabled { opacity: 0.65; cursor: not-allowed; }
.error { color: var(--danger); margin-top: 1rem; text-align: center; }
.server-status { display: flex; align-items: center; gap: 0.4rem; font-size: 0.8rem; color: var(--text-muted); cursor: default; padding: 0.35rem 0.55rem; border: 1px solid var(--border); border-radius: var(--radius-pill); background: var(--bg-app); }
.status-dot { width: 8px; height: 8px; border-radius: 50%; }
.status-dot.online { background: var(--success); }
.status-dot.offline { background: var(--danger); }
.status-dot.checking { background: var(--warn); }
.status-text { letter-spacing: 0.5px; }
.panel-region:focus { outline: none; }

@media (max-width: 768px) {
  .navbar { flex-direction: column; gap: 0.75rem; padding: 0.75rem 1rem; align-items: stretch; }
  .brand { justify-content: space-between; }
  .user-chip { max-width: 58vw; }
  .nav-links { width: 100%; overflow-x: auto; justify-content: flex-start; gap: 0.5rem; padding-bottom: 0.4rem; scrollbar-width: thin; }
  .nav-links button { flex: 0 0 auto; padding: 0.4rem 0.75rem; font-size: 0.85rem; }
  .server-status { width: fit-content; }
  .container { padding: 1rem; }
}
</style>




