<template>
  <div id="app">
    <!-- 离线提示条 -->
    <div v-if="!isOnline" class="offline-banner">
      网络连接已断开，请检查网络设置
    </div>

    <nav v-if="isLoggedIn" class="navbar">
      <div class="brand">
        <h1>AI Hub</h1>
        <span class="user-chip">{{ currentUser?.username }} · {{ roleText }}</span>
      </div>
      <div class="nav-links">
        <button @click="currentView = 'accounts'" :class="{ active: currentView === 'accounts' }">账户管理</button>
        <button @click="currentView = 'runs'" :class="{ active: currentView === 'runs' }">签到记录</button>
        <button @click="currentView = 'statistics'" :class="{ active: currentView === 'statistics' }">数据统计</button>
        <button @click="currentView = 'notifications'" :class="{ active: currentView === 'notifications' }">通知设置</button>
        <button @click="currentView = 'settings'" :class="{ active: currentView === 'settings' }" v-if="isAdmin">全局设置</button>
        <button @click="currentView = 'users'" :class="{ active: currentView === 'users' }" v-if="isAdmin">用户管理</button>
        <button @click="logout" class="btn-logout">退出</button>
      </div>
      <div
        class="server-status"
        :title="serverTime || '服务器时间'"
        @mouseenter="startHoverTimer"
        @mouseleave="stopHoverTimer"
      >
        <span class="status-dot" :class="serverOk ? 'online' : 'offline'"></span>
        <span class="status-text">{{ serverOk ? '在线' : '离线' }}</span>
      </div>
    </nav>

    <div class="container">
      <div v-if="authChecking" class="login-page">
        <div class="loading-panel">正在检查登录状态...</div>
      </div>

      <div v-else-if="!isLoggedIn" class="login-page">
        <form @submit.prevent="login" class="login-form">
          <h2>登录</h2>
          <div class="form-group">
            <input v-model="loginForm.username" placeholder="用户名" required :disabled="loginLoading" />
          </div>
          <div class="form-group">
            <input v-model="loginForm.password" type="password" placeholder="密码" required :disabled="loginLoading" />
          </div>
          <button type="submit" class="btn-primary" :disabled="loginLoading">
            {{ loginLoading ? '登录中...' : '登录' }}
          </button>
          <p v-if="error" class="error">{{ error }}</p>
        </form>
      </div>

      <div v-else>
        <AccountPanel v-if="currentView === 'accounts'" :current-user="currentUser" :is-admin="isAdmin" />
        <CheckinRunsPanel v-else-if="currentView === 'runs'" :current-user="currentUser" :is-admin="isAdmin" />
        <StatisticsPanel v-else-if="currentView === 'statistics'" :current-user="currentUser" :is-admin="isAdmin" />
        <NotificationPanel v-else-if="currentView === 'notifications'" />
        <SettingsPanel v-else-if="currentView === 'settings' && isAdmin" />
        <AdminUserPanel v-else-if="currentView === 'users'" :current-user="currentUser" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
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

const isLoggedIn = ref(false)
const currentUser = ref<AppUser | null>(null)
const currentView = ref('accounts')
const loginForm = ref({ username: '', password: '' })
const error = ref('')
const authChecking = ref(true)
const loginLoading = ref(false)
const serverOk = ref(true)
const serverTime = ref('')
const isOnline = ref(navigator.onLine)
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
  background: #f59e0b;
  color: #000;
  text-align: center;
  padding: 0.75rem;
  font-weight: 500;
  position: sticky;
  top: 0;
  z-index: 1000;
  box-shadow: 0 2px 4px rgba(0,0,0,0.2);
}
.navbar { background: rgba(17, 24, 39, 0.92); padding: 0.85rem 2rem; display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid #263241; position: sticky; top: 0; z-index: 30; backdrop-filter: blur(10px); gap: 1rem; }
.brand { display: flex; align-items: center; gap: 0.75rem; min-width: 0; }
.navbar h1 { font-size: 1.35rem; letter-spacing: 0; color: #f8fafc; white-space: nowrap; }
.user-chip { color: #cbd5e1; background: #0f172a; border: 1px solid #263241; border-radius: 999px; padding: 0.25rem 0.55rem; font-size: 0.78rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 220px; }
.nav-links { display: flex; gap: 0.4rem; align-items: center; }
.nav-links button { background: transparent; color: #cbd5e1; border: 1px solid transparent; padding: 0.48rem 0.8rem; cursor: pointer; border-radius: 6px; transition: all 0.16s ease; }
.nav-links button.active { background: #2563eb; border-color: #3b82f6; color: white; }
.nav-links button:hover:not(.active) { background: #1f2937; border-color: #334155; color: #fff; }
.btn-logout { background: #b91c1c; color: white; border: none; padding: 0.5rem 1rem; border-radius: 6px; cursor: pointer; }
.btn-logout:hover { background: #dc2626; }
.container { max-width: 1400px; margin: 0 auto; padding: 1.5rem; }
.login-page { display: flex; align-items: center; justify-content: center; min-height: 80vh; }
.loading-panel { background: #111827; border: 1px solid #263241; border-radius: 8px; color: #cbd5e1; padding: 1.2rem 1.5rem; }
.login-form { background: #111827; border: 1px solid #263241; padding: 2rem; border-radius: 8px; width: 100%; max-width: 400px; box-shadow: 0 24px 70px rgba(0, 0, 0, 0.35); }
.login-form h2 { margin-bottom: 1.5rem; text-align: center; color: #f8fafc; }
.form-group { margin-bottom: 1rem; }
.form-group input { width: 100%; padding: 0.75rem; background: #0b1220; border: 1px solid #334155; border-radius: 6px; color: #fff; font-size: 1rem; }
.btn-primary { width: 100%; background: #2563eb; color: white; border: none; padding: 0.75rem; border-radius: 6px; cursor: pointer; font-size: 1rem; font-weight: 600; }
.btn-primary:hover:not(:disabled) { background: #1d4ed8; }
.btn-primary:disabled { opacity: 0.65; cursor: not-allowed; }
.error { color: #ef4444; margin-top: 1rem; text-align: center; }
.server-status { display: flex; align-items: center; gap: 0.4rem; font-size: 0.8rem; color: #94a3b8; cursor: default; padding: 0.35rem 0.55rem; border: 1px solid #263241; border-radius: 999px; background: #0f172a; }
.status-dot { width: 8px; height: 8px; border-radius: 50%; }
.status-dot.online { background: #10b981; }
.status-dot.offline { background: #ef4444; }
.status-text { letter-spacing: 0.5px; }

@media (max-width: 768px) {
  .navbar { flex-direction: column; gap: 0.75rem; padding: 0.75rem 1rem; align-items: stretch; }
  .brand { justify-content: space-between; }
  .user-chip { max-width: 58vw; }
  .nav-links { width: 100%; overflow-x: auto; justify-content: flex-start; gap: 0.5rem; padding-bottom: 0.15rem; }
  .nav-links button { flex: 0 0 auto; }
  .nav-links button { padding: 0.4rem 0.75rem; font-size: 0.85rem; }
  .server-status { width: fit-content; }
  .container { padding: 1rem; }
}
</style>




