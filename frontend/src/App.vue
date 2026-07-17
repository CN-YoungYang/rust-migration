<template>
  <div id="app">
    <a class="skip-link" href="#main-content">跳到主要内容</a>

    <!-- 离线提示条 -->
    <div v-if="!isOnline" class="offline-banner" role="status" aria-live="polite">
      网络连接已断开，请检查网络设置
    </div>

    <div v-if="isLoggedIn" class="workspace-shell">
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
            <input id="login-username" v-model="loginForm.username" name="username" placeholder="输入用户名" autocomplete="username" autocapitalize="none" required :disabled="loginLoading" />
          </div>
          <div class="form-group">
            <label for="login-password">密码</label>
            <input id="login-password" v-model="loginForm.password" name="password" type="password" placeholder="输入密码" autocomplete="current-password" required :disabled="loginLoading" />
          </div>
          <button type="submit" class="btn-primary" :disabled="loginLoading">
            {{ loginLoading ? '正在登录' : '登录' }}
          </button>
          <p v-if="error" class="error" role="alert" aria-live="assertive">{{ error }}</p>
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

<style>
#app {
  position: relative;
  min-height: 100dvh;
  background: var(--bg-base);
  overflow-x: clip;
}
#app::before {
  content: '';
  position: fixed;
  width: 36rem;
  height: 36rem;
  right: -18rem;
  top: -18rem;
  z-index: 0;
  pointer-events: none;
  border: 1px solid var(--color-rule);
  border-radius: 50%;
}
.skip-link {
  position: fixed;
  top: 0.75rem;
  left: 0.75rem;
  z-index: 1100;
  padding: 0.65rem 0.9rem;
  border-radius: 6px;
  background: var(--text-strong);
  color: var(--color-accent-ink);
  font-weight: 600;
  transform: translateY(-180%);
  transition: transform var(--dur-short) var(--ease-out);
}
.skip-link:focus { transform: translateY(0); }
.offline-banner {
  background: var(--warn-soft);
  color: var(--warn);
  text-align: center;
  padding: 0.65rem;
  font-weight: 600;
  position: sticky;
  top: 0;
  z-index: 1000;
  border-bottom: 1px solid var(--warn-border);
}
.workspace-shell {
  position: relative;
  z-index: 1;
  display: grid;
  grid-template-columns: 17rem minmax(0, 1fr);
  min-height: 100dvh;
}
.navbar {
  position: sticky;
  top: 0;
  height: 100dvh;
  display: flex;
  flex-direction: column;
  gap: 2.5rem;
  padding: 1.5rem 1.25rem;
  background: var(--bg-app);
  border-right: 1px solid var(--border);
  z-index: 30;
}
.brand,
.login-brand { display: flex; align-items: center; gap: 0.8rem; min-width: 0; }
.brand-mark {
  width: 2.5rem;
  height: 2.5rem;
  display: grid;
  place-items: center;
  flex: 0 0 auto;
  border-radius: 8px;
  background: var(--text-strong);
  color: var(--color-accent-ink);
  font: 700 0.72rem/1 var(--font-mono);
  letter-spacing: 0.08em;
}
.navbar h1 { color: var(--text-strong); font-size: var(--text-nav); letter-spacing: -0.02em; white-space: nowrap; }
.brand p { margin-top: 0.1rem; color: var(--text-muted); font-size: var(--text-meta); }
.nav-links { display: grid; gap: 0.35rem; }
.nav-links button {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 0.7rem;
  background: transparent;
  color: var(--text-muted);
  border: 1px solid transparent;
  padding: 0.7rem 0.75rem;
  text-align: left;
  border-radius: 6px;
  font-weight: 550;
}
.nav-mark { width: 0.42rem; height: 0.42rem; flex: 0 0 auto; border: 1px solid currentColor; border-radius: 2px; }
.nav-links button.active { background: var(--accent-soft); border-color: var(--border); color: var(--text-strong); }
.nav-links button.active .nav-mark { background: var(--text-strong); border-color: var(--text-strong); }
.nav-links button:hover:not(.active) { background: var(--bg-elevated); color: var(--text-strong); }
.nav-footer { display: grid; gap: 0.75rem; margin-top: auto; }
.server-status,
.user-card { border: 1px solid var(--border); background: var(--bg-card); border-radius: 8px; }
.server-status { display: flex; align-items: center; gap: 0.65rem; color: var(--text-muted); padding: 0.7rem 0.75rem; cursor: default; }
.server-status > span:last-child { display: grid; gap: 0.05rem; }
.server-status small,
.user-meta small { color: var(--text-muted); font-size: 0.7rem; }
.status-text { color: var(--text-strong); font-size: 0.82rem; letter-spacing: 0.02em; }
.status-dot { width: 8px; height: 8px; border-radius: 50%; }
.status-dot.online { background: var(--success); }
.status-dot.offline { background: var(--danger); }
.status-dot.checking { background: var(--warn); }
.user-card { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: 0.65rem; padding: 0.65rem; }
.user-avatar { width: 2rem; height: 2rem; display: grid; place-items: center; border-radius: 6px; background: var(--pale-blue); color: var(--pale-blue-text); font-weight: 700; }
.user-meta { min-width: 0; display: grid; }
.user-meta strong { overflow: hidden; color: var(--text-strong); font-size: var(--text-meta); text-overflow: ellipsis; white-space: nowrap; }
.btn-logout { background: transparent; color: var(--danger); border: 0; padding: 0.35rem; font-size: 0.74rem; }
.btn-logout:hover:not(:disabled) { background: var(--danger-soft); }
.container { width: 100%; max-width: 1520px; margin: 0 auto; padding: clamp(1.25rem, 3vw, 3rem); }
.workspace-main { min-width: 0; }
.workspace-heading {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 2rem;
  padding: 0.5rem 0 clamp(1.75rem, 4vw, 3.5rem);
  border-bottom: 1px solid var(--border);
}
.workspace-heading > div > p,
.login-kicker { color: var(--text-muted); font: 600 0.72rem/1.4 var(--font-mono); letter-spacing: 0.08em; text-transform: uppercase; }
.workspace-heading h2 { margin-top: 0.55rem; font: 500 clamp(2rem, 4vw, 3.6rem)/1 var(--font-editorial); color: var(--text-strong); letter-spacing: -0.045em; }
.workspace-description { max-width: 34rem; color: var(--text-muted); text-align: right; }
.panel-region:focus { outline: none; }
.auth-container { position: relative; z-index: 1; max-width: 1500px; }
.login-page { display: grid; grid-template-columns: minmax(0, 1.35fr) minmax(22rem, 0.65fr); align-items: stretch; min-height: calc(100dvh - 6rem); padding: clamp(1rem, 4vw, 4rem) 0; }
.loading-panel { grid-column: 1 / -1; place-self: center; background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius); color: var(--text-faint); padding: 1.2rem 1.5rem; }
.login-intro { display: flex; flex-direction: column; justify-content: space-between; gap: 5rem; padding: clamp(1rem, 5vw, 5rem) clamp(2rem, 7vw, 7rem) clamp(2rem, 5vw, 5rem) 0; }
.login-brand { color: var(--text-strong); font-weight: 700; letter-spacing: -0.02em; }
.login-copy { max-width: 58rem; }
.login-copy h1 { max-width: 15ch; margin: 1.25rem 0 1.5rem; font: 500 clamp(3rem, 6vw, 6.8rem)/0.94 var(--font-editorial); color: var(--text-strong); letter-spacing: -0.055em; text-wrap: balance; }
.login-copy > p:last-child { max-width: 47rem; color: var(--text-muted); font-size: clamp(1rem, 1.5vw, 1.18rem); line-height: 1.75; }
.login-capabilities { display: grid; grid-template-columns: minmax(0, 1.25fr) repeat(2, minmax(0, 0.875fr)); border-top: 1px solid var(--border); border-bottom: 1px solid var(--border); }
.login-capabilities span { padding: 1rem 0; color: var(--text); font: 600 0.76rem/1.4 var(--font-mono); letter-spacing: 0.05em; }
.login-capabilities span + span { padding-left: 1rem; border-left: 1px solid var(--border); }
.login-form { align-self: center; background: var(--bg-card); border: 1px solid var(--border); padding: clamp(1.75rem, 4vw, 3rem); border-radius: 12px; width: 100%; max-width: 30rem; box-shadow: var(--shadow-modal); }
.login-heading { margin-bottom: 1.75rem; }
.login-kicker { display: block; margin-bottom: 0.75rem; }
.login-form h2 { margin-bottom: 0.55rem; color: var(--text-strong); font: 500 clamp(2rem, 4vw, 3rem)/1 var(--font-editorial); letter-spacing: -0.04em; }
.login-heading p { max-width: 32ch; color: var(--text-muted); font-size: 0.92rem; }
.form-group { margin-bottom: 1rem; }
.form-group label { display: block; margin-bottom: 0.45rem; color: var(--text); font-size: 0.82rem; font-weight: 600; }
.form-group input { width: 100%; padding: 0.82rem 0.9rem; background: var(--bg-well); border: 1px solid var(--border-input); border-radius: 6px; color: var(--text-strong); font-size: 0.95rem; transition: background-color var(--dur-short) var(--ease-out), border-color var(--dur-short) var(--ease-out); }
.form-group input:hover:not(:disabled) { border-color: var(--border-hover); }
.form-group input:focus { background: var(--bg-well); border-color: var(--accent-border); box-shadow: 0 0 0 3px var(--accent-soft); }
.btn-primary { width: 100%; background: var(--accent); color: var(--color-accent-ink); border: 1px solid var(--accent-border); padding: 0.82rem; border-radius: 6px; cursor: pointer; font-size: 0.95rem; font-weight: 700; box-shadow: none; }
.btn-primary:hover:not(:disabled) { background: var(--accent-hover); }
.btn-primary:disabled { opacity: 0.65; cursor: not-allowed; }
.error { color: var(--danger); margin-top: 1rem; text-align: left; font-size: 0.85rem; }
.login-footnote { margin-top: 1.25rem; color: var(--text-muted); font-size: 0.74rem; line-height: 1.6; }

@media (max-width: 980px) {
  .workspace-shell { grid-template-columns: 1fr; }
  .navbar { position: sticky; height: auto; padding: 0.75rem 1rem; gap: 0.75rem; border-right: 0; border-bottom: 1px solid var(--border); }
  .brand p,
  .nav-footer .server-status { display: none; }
  .nav-links { display: flex; gap: 0.35rem; overflow-x: auto; padding-bottom: 0.2rem; scrollbar-width: thin; }
  .nav-links button { width: auto; flex: 0 0 auto; padding: 0.55rem 0.7rem; }
  .nav-footer { position: absolute; top: 0.75rem; right: 1rem; }
  .user-card { border: 0; background: transparent; padding: 0; }
  .user-avatar,
  .user-meta small { display: none; }
  .workspace-heading { padding-top: 0; }
  .login-page { grid-template-columns: 1fr; min-height: auto; }
  .login-intro { padding: 2rem 0 4rem; gap: 4rem; }
  .login-form { max-width: none; }
}

@media (max-width: 640px) {
  .container { padding: 1rem; }
  .navbar { padding-right: 0.75rem; }
  .brand { padding-right: 7.5rem; }
  .brand-mark { width: 2.25rem; height: 2.25rem; }
  .nav-mark { display: none; }
  .workspace-heading { display: grid; gap: 0.8rem; }
  .workspace-description { text-align: left; }
  .login-intro { padding-top: 1rem; }
  .login-copy h1 { max-width: 12ch; font-size: clamp(2.8rem, 15vw, 4.5rem); }
  .login-capabilities { grid-template-columns: 1fr; }
  .login-capabilities span + span { padding-left: 0; border-left: 0; border-top: 1px solid var(--border); }
}
</style>




