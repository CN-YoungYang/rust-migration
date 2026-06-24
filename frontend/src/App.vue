<template>
  <div id="app">
    <!-- 离线提示条 -->
    <div v-if="!isOnline" class="offline-banner">
      ⚠️ 网络连接已断开，请检查网络设置
    </div>

    <nav v-if="isLoggedIn" class="navbar">
      <h1>AI Hub</h1>
      <div class="nav-links">
        <button @click="currentView = 'accounts'" :class="{ active: currentView === 'accounts' }">账户管理</button>
        <button @click="currentView = 'runs'" :class="{ active: currentView === 'runs' }">签到记录</button>
        <button @click="currentView = 'settings'" :class="{ active: currentView === 'settings' }" v-if="isAdmin">全局设置</button>
        <button @click="currentView = 'users'" :class="{ active: currentView === 'users' }" v-if="isAdmin">用户管理</button>
        <button @click="logout" class="btn-logout">退出</button>
      </div>
      <div class="server-status" :title="serverTime || '服务器时间'">
        <span class="status-dot" :class="serverOk ? 'online' : 'offline'"></span>
        <span class="status-text">{{ serverOk ? '在线' : '离线' }}</span>
      </div>
    </nav>

    <div class="container">
      <div v-if="!isLoggedIn" class="login-page">
        <form @submit.prevent="login" class="login-form">
          <h2>登录</h2>
          <div class="form-group">
            <input v-model="loginForm.username" placeholder="用户名" required />
          </div>
          <div class="form-group">
            <input v-model="loginForm.password" type="password" placeholder="密码" required />
          </div>
          <button type="submit" class="btn-primary">登录</button>
          <p v-if="error" class="error">{{ error }}</p>
        </form>
      </div>

      <div v-else>
        <AccountPanel v-if="currentView === 'accounts'" :current-user="currentUser" :is-admin="isAdmin" />
        <CheckinRunsPanel v-else-if="currentView === 'runs'" :current-user="currentUser" :is-admin="isAdmin" />
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
import SettingsPanel from './components/SettingsPanel.vue'
import AdminUserPanel from './components/AdminUserPanel.vue'
import { apiUrl, getToken, request } from './utils/api'
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
const serverOk = ref(true)
const serverTime = ref('')
const isOnline = ref(navigator.onLine)
let serverTimeOffset = 0 // 服务器时间与本地时间的差值（毫秒）
let timeTimer: ReturnType<typeof setInterval> | null = null

const isAdmin = computed(() => {
  return currentUser.value?.role === 'ADMIN' || currentUser.value?.role === 'SUPER_ADMIN'
})

const login = async () => {
  error.value = ''
  try {
    const res = await request(apiUrl('/auth/login'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(loginForm.value)
    })
    const data = await res.json()
    localStorage.setItem('token', data.token)
    await fetchCurrentUser()
    isLoggedIn.value = true
  } catch (e) {
    error.value = e instanceof Error ? e.message : '登录失败'
  }
}

const fetchCurrentUser = async () => {
  const token = getToken()
  if (!token) return

  try {
    const res = await request(apiUrl('/auth/me'), {
      headers: { 'Authorization': `Bearer ${token}` }
    })
    const data = await res.json()
    currentUser.value = data.user
    isLoggedIn.value = !!data.user
  } catch {
    localStorage.removeItem('token')
    isLoggedIn.value = false
    currentUser.value = null
  }
}

const logout = async () => {
  const token = getToken()
  if (token) {
    try {
      await request(apiUrl('/auth/logout'), {
        method: 'POST',
        headers: { 'Authorization': `Bearer ${token}` }
      })
    } catch {
      // 本地退出优先，不阻塞用户操作
    }
  }
  localStorage.removeItem('token')
  isLoggedIn.value = false
  currentUser.value = null
  currentView.value = 'accounts'
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
      const data = await res.json()
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

onMounted(() => {
  fetchCurrentUser()
  checkHealth()
  fetchServerTime()
  // 健康检查：每 5 分钟（降低频率，减少不必要的请求）
  healthTimer = setInterval(checkHealth, 300000)
  // 服务器时间：每 10 秒更新一次显示（降低 CPU 占用）
  timeTimer = setInterval(updateDisplayTime, 10000)

  // 离线检测
  window.addEventListener('online', () => {
    isOnline.value = true
    checkHealth() // 恢复在线时立即检查健康状态
  })
  window.addEventListener('offline', () => {
    isOnline.value = false
  })
})

onUnmounted(() => {
  if (healthTimer) clearInterval(healthTimer)
  if (timeTimer) clearInterval(timeTimer)
  window.removeEventListener('online', () => { isOnline.value = true })
  window.removeEventListener('offline', () => { isOnline.value = false })
})
</script>

<style>
#app { min-height: 100vh; }
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
.navbar { background: #1a1a1a; padding: 1rem 2rem; display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid #333; }
.navbar h1 { font-size: 1.5rem; }
.nav-links { display: flex; gap: 1rem; }
.nav-links button { background: transparent; color: #ccc; border: none; padding: 0.5rem 1rem; cursor: pointer; border-radius: 4px; transition: all 0.2s; }
.nav-links button.active { background: #0070f3; color: white; }
.nav-links button:hover:not(.active) { background: #2a2a2a; }
.btn-logout { background: #ef4444; color: white; border: none; padding: 0.5rem 1rem; border-radius: 4px; cursor: pointer; }
.btn-logout:hover { background: #dc2626; }
.container { max-width: 1400px; margin: 0 auto; padding: 2rem; }
.login-page { display: flex; align-items: center; justify-content: center; min-height: 80vh; }
.login-form { background: #1a1a1a; padding: 2rem; border-radius: 8px; width: 100%; max-width: 400px; }
.login-form h2 { margin-bottom: 1.5rem; text-align: center; }
.form-group { margin-bottom: 1rem; }
.form-group input { width: 100%; padding: 0.75rem; background: #2a2a2a; border: 1px solid #444; border-radius: 4px; color: #fff; font-size: 1rem; }
.btn-primary { width: 100%; background: #0070f3; color: white; border: none; padding: 0.75rem; border-radius: 4px; cursor: pointer; font-size: 1rem; font-weight: 500; }
.btn-primary:hover { background: #0051cc; }
.error { color: #ef4444; margin-top: 1rem; text-align: center; }
.server-status { display: flex; align-items: center; gap: 0.4rem; font-size: 0.8rem; color: #9ca3af; cursor: default; }
.status-dot { width: 8px; height: 8px; border-radius: 50%; }
.status-dot.online { background: #10b981; }
.status-dot.offline { background: #ef4444; }
.status-text { letter-spacing: 0.5px; }

@media (max-width: 768px) {
  .navbar { flex-direction: column; gap: 0.75rem; padding: 0.75rem 1rem; }
  .nav-links { flex-wrap: wrap; justify-content: center; gap: 0.5rem; }
  .nav-links button { padding: 0.4rem 0.75rem; font-size: 0.85rem; }
  .container { padding: 1rem; }
}
</style>




