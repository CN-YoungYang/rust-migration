<template>
  <div id="app">
    <nav v-if="isLoggedIn" class="navbar">
      <h1>AI Hub</h1>
      <div class="nav-links">
        <button @click="currentView = 'accounts'" :class="{ active: currentView === 'accounts' }">账户管理</button>
        <button @click="currentView = 'runs'" :class="{ active: currentView === 'runs' }">签到记录</button>
        <button @click="currentView = 'settings'" :class="{ active: currentView === 'settings' }" v-if="isAdmin">全局设置</button>
        <button @click="currentView = 'users'" :class="{ active: currentView === 'users' }" v-if="isAdmin">用户管理</button>
        <button @click="logout" class="btn-logout">退出</button>
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
import { ref, computed, onMounted } from 'vue'
import AccountPanel from './components/AccountPanel.vue'
import CheckinRunsPanel from './components/CheckinRunsPanel.vue'
import SettingsPanel from './components/SettingsPanel.vue'
import AdminUserPanel from './components/AdminUserPanel.vue'
import { apiUrl, getToken } from './utils/api'
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

const isAdmin = computed(() => {
  return currentUser.value?.role === 'ADMIN' || currentUser.value?.role === 'SUPER_ADMIN'
})

const login = async () => {
  error.value = ''
  try {
    const res = await fetch(apiUrl('/auth/login'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(loginForm.value)
    })
    if (res.ok) {
      const data = await res.json()
      localStorage.setItem('token', data.token)
      await fetchCurrentUser()
      isLoggedIn.value = true
    } else {
      error.value = '登录失败'
    }
  } catch (e) {
    error.value = '网络错误'
  }
}

const fetchCurrentUser = async () => {
  const token = getToken()
  if (!token) return

  const res = await fetch(apiUrl('/auth/me'), {
    headers: { 'Authorization': `Bearer ${token}` }
  })
  if (res.ok) {
    const data = await res.json()
    currentUser.value = data.user
    isLoggedIn.value = !!data.user
  } else {
    localStorage.removeItem('token')
    isLoggedIn.value = false
    currentUser.value = null
  }
}

const logout = async () => {
  const token = getToken()
  if (token) {
    try {
      await fetch(apiUrl('/auth/logout'), {
        method: 'POST',
        headers: { 'Authorization': `Bearer ${token}` }
      })
    } catch (e) {
      // 本地退出优先，不阻塞用户操作
    }
  }
  localStorage.removeItem('token')
  isLoggedIn.value = false
  currentUser.value = null
  currentView.value = 'accounts'
}

onMounted(fetchCurrentUser)
</script>

<style>
#app { min-height: 100vh; }
.navbar { background: #1a1a1a; padding: 1rem 2rem; display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid #333; }
.navbar h1 { font-size: 1.5rem; }
.nav-links { display: flex; gap: 1rem; }
.nav-links button { background: transparent; color: #ccc; border: none; padding: 0.5rem 1rem; cursor: pointer; border-radius: 4px; transition: all 0.2s; }
.nav-links button.active { background: #0070f3; color: white; }
.nav-links button:hover:not(.active) { background: #2a2a2a; }
.btn-logout { background: #ef4444 !important; color: white !important; }
.btn-logout:hover { background: #dc2626 !important; }
.container { max-width: 1400px; margin: 0 auto; padding: 2rem; }
.login-page { display: flex; align-items: center; justify-content: center; min-height: 80vh; }
.login-form { background: #1a1a1a; padding: 2rem; border-radius: 8px; width: 100%; max-width: 400px; }
.login-form h2 { margin-bottom: 1.5rem; text-align: center; }
.form-group { margin-bottom: 1rem; }
.form-group input { width: 100%; padding: 0.75rem; background: #2a2a2a; border: 1px solid #444; border-radius: 4px; color: #fff; font-size: 1rem; }
.btn-primary { width: 100%; background: #0070f3; color: white; border: none; padding: 0.75rem; border-radius: 4px; cursor: pointer; font-size: 1rem; font-weight: 500; }
.btn-primary:hover { background: #0051cc; }
.error { color: #ef4444; margin-top: 1rem; text-align: center; }
</style>




