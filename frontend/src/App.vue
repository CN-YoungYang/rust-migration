<template>
  <div id="app">
    <a class="skip-link" href="#main-content">跳到主要内容</a>

    <!-- 离线提示条 -->
    <div v-if="!isOnline" class="offline-banner" role="status" aria-live="polite">
      网络连接已断开，请检查网络设置
    </div>

    <div v-if="isLoggedIn" class="workspace-shell" :class="{ 'has-offline-banner': !isOnline }">
      <nav class="navbar" aria-label="主导航">
        <div class="brand">
          <span class="brand-mark" aria-hidden="true">AH</span>
          <div>
            <h1>AI Hub</h1>
            <p>自动签到控制台</p>
          </div>
        </div>

        <div class="nav-links" aria-label="功能导航">
          <button @click="selectView('accounts')" :class="{ active: currentView === 'accounts' }" :aria-current="currentView === 'accounts' ? 'page' : undefined"><span class="nav-mark" aria-hidden="true"></span>账户管理</button>
          <button @click="selectView('runs')" :class="{ active: currentView === 'runs' }" :aria-current="currentView === 'runs' ? 'page' : undefined"><span class="nav-mark" aria-hidden="true"></span>签到记录</button>
          <button @click="selectView('statistics')" :class="{ active: currentView === 'statistics' }" :aria-current="currentView === 'statistics' ? 'page' : undefined"><span class="nav-mark" aria-hidden="true"></span>数据统计</button>
          <button @click="selectView('notifications')" :class="{ active: currentView === 'notifications' }" :aria-current="currentView === 'notifications' ? 'page' : undefined"><span class="nav-mark" aria-hidden="true"></span>通知设置</button>
          <button @click="selectView('settings')" :class="{ active: currentView === 'settings' }" v-if="isAdmin" :aria-current="currentView === 'settings' ? 'page' : undefined"><span class="nav-mark" aria-hidden="true"></span>全局设置</button>
          <button @click="selectView('users')" :class="{ active: currentView === 'users' }" v-if="isAdmin" :aria-current="currentView === 'users' ? 'page' : undefined"><span class="nav-mark" aria-hidden="true"></span>用户管理</button>
        </div>

        <div class="nav-footer">
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
            <span>
              <small>服务状态</small>
              <strong class="status-text">{{ serverStatusText }}</strong>
            </span>
          </div>
          <div class="user-card">
            <span class="user-avatar" aria-hidden="true">{{ userInitial }}</span>
            <span class="user-meta">
              <strong>{{ currentUser?.username }}</strong>
              <small>{{ roleText }}</small>
            </span>
            <button @click="logout" class="btn-logout">退出</button>
          </div>
        </div>
      </nav>

      <main id="main-content" class="container workspace-main" tabindex="-1">
        <header class="workspace-heading">
          <div>
            <p>运行工作台</p>
            <h2>{{ currentViewLabel }}</h2>
          </div>
          <p class="workspace-description">{{ currentViewDescription }}</p>
        </header>

        <div
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

    <main v-else id="main-content" class="container auth-container" tabindex="-1">
      <div v-if="authChecking" class="login-page">
        <div class="loading-panel" role="status" aria-live="polite" aria-busy="true">正在检查登录状态...</div>
      </div>

      <div v-else class="login-page">
        <section class="login-intro" aria-labelledby="login-intro-title">
          <div class="login-brand"><span class="brand-mark" aria-hidden="true">AH</span><span>AI Hub</span></div>
          <div class="login-copy">
            <p class="login-kicker">多站点签到管理</p>
            <h1 id="login-intro-title">把重复签到，交给一个安静可靠的工作台。</h1>
            <p>集中管理 New API 兼容站点账户、执行记录、余额与通知。面向低资源服务器设计，保持清晰、稳定、可追踪。</p>
          </div>
          <div class="login-capabilities" aria-label="平台能力">
            <span>批量执行</span>
            <span>失败重试</span>
            <span>定时调度</span>
          </div>
        </section>

        <form @submit.prevent="login" class="login-form" aria-labelledby="login-title">
          <div class="login-heading">
            <span class="login-kicker">进入控制台</span>
            <h2 id="login-title">欢迎回来</h2>
            <p>使用管理员创建的账户继续。</p>
          </div>
          <div class="form-group">
            <label for="login-username">用户名</label>
            <input id="login-username" v-model="loginForm.username" name="username" placeholder="输入用户名" autocomplete="username" autocapitalize="none" required :disabled="loginLoading" :aria-invalid="Boolean(error)" aria-describedby="login-error" />
          </div>
          <div class="form-group">
            <label for="login-password">密码</label>
            <input id="login-password" v-model="loginForm.password" name="password" type="password" placeholder="输入密码" autocomplete="current-password" required :disabled="loginLoading" :aria-invalid="Boolean(error)" aria-describedby="login-error" />
          </div>
          <button type="submit" class="btn-primary" :disabled="loginLoading">
            {{ loginLoading ? '正在登录' : '登录' }}
          </button>
          <p id="login-error" class="error field-error-slot" :class="{ 'is-empty': !error }" :role="error ? 'alert' : undefined" aria-live="assertive">{{ error || '\u00a0' }}</p>
          <p class="login-footnote">会话通过 HttpOnly Cookie 保存，凭据不会存储在浏览器本地。</p>
        </form>
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

const viewDescriptions: Record<ViewName, string> = {
  accounts: '管理站点凭据、余额与批量签到任务。',
  runs: '查看每次执行结果、失败原因与重试状态。',
  statistics: '按时间和站点观察成功率、余额与运行趋势。',
  notifications: '配置邮件、Webhook 与 Telegram 通知。',
  settings: '调整全局调度窗口、重试规则与清理策略。',
  users: '维护用户状态、角色与平台访问权限。',
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
const currentViewDescription = computed(() => viewDescriptions[currentView.value])
const userInitial = computed(() => currentUser.value?.username?.trim().slice(0, 1).toUpperCase() || 'U')
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

<!-- 样式归属：app-shell 与登录页共享设计语言全部位于 src/workbench.css（design.md 约定）。
     本组件不再持有样式块，避免与 workbench.css 双头维护。 -->
