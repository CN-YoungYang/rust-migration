<template>
  <div id="app" class="app-root">
    <!-- 未登录：登录视图 -->
    <div v-if="!isLoggedIn" class="login-page" :style="{ background: themeVars.bodyColor }">
      <div v-if="!isOnline" class="offline-strip" role="status" aria-live="polite" :style="offlineStripStyle">
        网络连接已断开，请检查网络设置
      </div>
      <div v-if="authChecking" class="login-center">
        <n-spin size="large" />
        <p class="muted login-loading-text">正在检查登录状态…</p>
      </div>
      <div v-else class="login-center">
        <n-card class="login-card" role="main" aria-labelledby="login-title">
          <div class="login-brand">
            <span class="brand-mark" aria-hidden="true">AH</span>
            <div>
              <h1 id="login-title" class="login-title">AI Hub</h1>
              <p class="login-subtitle muted">多站点自动签到管理</p>
            </div>
          </div>
          <n-form
            ref="loginFormRef"
            :model="loginForm"
            :rules="loginRules"
            class="login-form"
            @keydown.enter.prevent="login"
          >
            <n-form-item label="用户名" path="username">
              <n-input
                v-model:value="loginForm.username"
                placeholder="输入用户名"
                autocomplete="username"
                autocapitalize="none"
                :disabled="loginLoading"
              />
            </n-form-item>
            <n-form-item label="密码" path="password">
              <n-input
                v-model:value="loginForm.password"
                type="password"
                placeholder="输入密码"
                autocomplete="current-password"
                show-password-on="click"
                :disabled="loginLoading"
              />
            </n-form-item>
            <n-alert v-if="error" type="error" :show-icon="true" class="login-error" role="alert" aria-live="assertive">
              {{ error }}
            </n-alert>
            <n-button type="primary" size="large" block :loading="loginLoading" @click="login">
              {{ loginLoading ? '正在登录' : '登录' }}
            </n-button>
            <p class="login-footnote muted">会话通过 HttpOnly Cookie 保存，凭据不会存储在浏览器本地。</p>
          </n-form>
        </n-card>
      </div>
    </div>

    <!-- 已登录：工作台 -->
    <n-layout v-else has-sider class="app-layout" :style="{ height: '100vh' }">
      <n-layout-sider
        bordered
        collapse-mode="width"
        :collapsed-width="64"
        :width="220"
        :collapsed="collapsed"
        show-trigger
        class="app-sider"
        @collapse="collapsed = true"
        @expand="collapsed = false"
      >
        <div class="sider-brand" :class="{ collapsed }">
          <span class="brand-mark" aria-hidden="true">AH</span>
          <span v-if="!collapsed" class="sider-title">AI Hub</span>
        </div>
        <n-menu
          class="sider-menu"
          :collapsed="collapsed"
          :collapsed-width="64"
          :collapsed-icon-size="20"
          :options="menuOptions"
          :value="currentView"
          @update:value="selectView"
        />
      </n-layout-sider>

      <n-layout class="app-column">
        <div v-if="!isOnline" class="offline-strip" role="status" aria-live="polite" :style="offlineStripStyle">
          网络连接已断开，请检查网络设置
        </div>
        <n-layout-header bordered class="app-header">
          <div class="header-title">
            <p class="header-kicker muted">运行工作台</p>
            <h1 class="header-h1">{{ currentViewLabel }}</h1>
            <span class="header-desc muted">{{ currentViewDescription }}</span>
          </div>
          <n-space align="center" :size="14">
            <n-tooltip trigger="hover" @update:show="onTooltipShowChange">
              <template #trigger>
                <n-tag :type="serverTagType" round :bordered="false" :style="{ cursor: 'default' }">{{ serverStatusText }}</n-tag>
              </template>
              <span>服务器时间：{{ serverTime || '获取中…' }}</span>
            </n-tooltip>
            <n-button
              quaternary
              circle
              :title="isDark ? '切换到亮色模式' : '切换到暗色模式'"
              aria-label="切换主题"
              @click="$emit('toggle-dark')"
            >
              <template #icon><n-icon :component="isDark ? SunnyOutline : MoonOutline" /></template>
            </n-button>
            <n-dropdown :options="userMenuOptions" trigger="click" @select="onUserMenuSelect">
              <n-button quaternary class="user-btn">
                <n-avatar round :size="28" class="user-avatar" :style="{ background: themeVars.primaryColor }">{{ userInitial }}</n-avatar>
                <span class="user-name">{{ currentUser?.username }}</span>
                <n-icon :component="ChevronDownOutline" />
              </n-button>
            </n-dropdown>
          </n-space>
        </n-layout-header>

        <n-layout-content class="app-content" content-style="padding: 20px 28px; min-height: 0;">
          <div
            ref="panelRegion"
            class="panel-region"
            role="region"
            :aria-label="`${currentViewLabel}面板`"
            tabindex="-1"
          >
            <Transition name="panel-fade" mode="out-in">
              <KeepAlive :include="cachedPanelNames">
                <component :is="activePanelComponent" v-bind="activePanelProps" />
              </KeepAlive>
            </Transition>
          </div>
        </n-layout-content>
      </n-layout>
    </n-layout>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, nextTick, onMounted, onUnmounted, h, type Component } from 'vue'
import {
  NLayout,
  NLayoutSider,
  NLayoutHeader,
  NLayoutContent,
  NMenu,
  NButton,
  NIcon,
  NTag,
  NTooltip,
  NDropdown,
  NSpace,
  NCard,
  NForm,
  NFormItem,
  NInput,
  NSpin,
  NAlert,
  NAvatar,
  useMessage,
  useThemeVars,
  type FormRules,
  type FormInst,
  type MenuOption,
} from 'naive-ui'
import {
  ChevronDownOutline,
  FingerPrintOutline,
  ListOutline,
  LogOutOutline,
  MoonOutline,
  NotificationsOutline,
  PeopleOutline,
  SettingsOutline,
  StatsChartOutline,
  SunnyOutline,
} from '@vicons/ionicons5'
import AccountPanel from './components/AccountPanel.vue'
import CheckinRunsPanel from './components/CheckinRunsPanel.vue'
import StatisticsPanel from './components/StatisticsPanel.vue'
import NotificationPanel from './components/NotificationPanel.vue'
import SettingsPanel from './components/SettingsPanel.vue'
import AdminUserPanel from './components/AdminUserPanel.vue'
import { AUTH_EXPIRED_EVENT, apiUrl, request, responseData } from './utils/api'

interface AppUser {
  id: string
  username: string
  role: string
  enabled: boolean
}

type ViewName = 'accounts' | 'runs' | 'statistics' | 'notifications' | 'settings' | 'users'

const props = defineProps<{ isDark: boolean }>()
defineEmits<{ 'toggle-dark': [] }>()

const message = useMessage()
const themeVars = useThemeVars()

const offlineStripStyle = computed(() => ({
  background: `color-mix(in srgb, ${themeVars.value.warningColor} 12%, transparent)`,
  color: themeVars.value.warningColor,
}))

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

function renderIcon(icon: Component) {
  return () => h(NIcon, null, { default: () => h(icon) })
}

const menuIconMap: Record<ViewName, Component> = {
  accounts: FingerPrintOutline,
  runs: ListOutline,
  statistics: StatsChartOutline,
  notifications: NotificationsOutline,
  settings: SettingsOutline,
  users: PeopleOutline,
}

const isLoggedIn = ref(false)
const currentUser = ref<AppUser | null>(null)
const currentView = ref<ViewName>('accounts')
const collapsed = ref(false)
const loginForm = ref({ username: '', password: '' })
const loginFormRef = ref<FormInst | null>(null)
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

const loginRules: FormRules = {
  username: { required: true, message: '请输入用户名', trigger: ['blur', 'input'] },
  password: { required: true, message: '请输入密码', trigger: ['blur', 'input'] },
}

const isAdmin = computed(() => {
  return currentUser.value?.role === 'ADMIN' || currentUser.value?.role === 'SUPER_ADMIN'
})

const roleText = computed(() => {
  const map: Record<string, string> = {
    USER: '普通用户',
    ADMIN: '管理员',
    SUPER_ADMIN: '超级管理员',
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
const serverTagType = computed<'default' | 'success' | 'error'>(() => {
  if (serverOk.value === null) return 'default'
  return serverOk.value ? 'success' : 'error'
})

const menuOptions = computed<MenuOption[]>(() => {
  const views: ViewName[] = ['accounts', 'runs', 'statistics', 'notifications']
  if (isAdmin.value) views.push('settings', 'users')
  return views.map((view) => ({
    key: view,
    label: viewLabels[view],
    icon: renderIcon(menuIconMap[view]),
  }))
})

const userMenuOptions = computed<MenuOption[]>(() => [
  {
    key: 'role',
    label: roleText.value,
    disabled: true,
  },
  {
    key: 'logout',
    label: '退出登录',
    icon: renderIcon(LogOutOutline),
  },
])

const selectView = (view: ViewName) => {
  if (currentView.value === view) return
  currentView.value = view
  void nextTick(() => panelRegion.value?.focus())
}

const onUserMenuSelect = (key: string) => {
  if (key === 'logout') void logout()
}

const login = async () => {
  if (loginLoading.value) return
  error.value = ''
  try {
    await loginFormRef.value?.validate()
  } catch {
    return
  }
  loginLoading.value = true
  try {
    const res = await request(apiUrl('/auth/login'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(loginForm.value),
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
  if (hoverTimer) return
  updateDisplayTime() // 立即更新一次
  hoverTimer = setInterval(updateDisplayTime, 1000)
}

const stopHoverTimer = () => {
  if (hoverTimer) {
    clearInterval(hoverTimer)
    hoverTimer = null
  }
}

const onTooltipShowChange = (show: boolean) => {
  if (show) startHoverTimer()
  else stopHoverTimer()
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
  message.error('登录已过期，请重新登录')
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

<style scoped>
.login-page {
  display: flex;
  flex-direction: column;
  min-height: 100vh;
}

.login-center {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 24px;
  background: radial-gradient(
    ellipse 90% 60% at 50% -10%,
    color-mix(in srgb, v-bind('themeVars.primaryColor') 8%, transparent),
    transparent 70%
  );
}

.login-loading-text {
  margin: 0;
}

.login-card {
  width: min(400px, 100%);
}

.login-brand {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 20px;
}

.login-title {
  margin: 0;
  font-size: 22px;
  font-weight: 700;
  line-height: 1.2;
}

.login-subtitle {
  margin: 2px 0 0;
  font-size: 13px;
}

.login-error {
  margin-bottom: 16px;
}

.login-form {
  width: 100%;
}

.login-footnote {
  margin: 14px 0 0;
  font-size: 12px;
  text-align: center;
}

.brand-mark {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: 10px;
  font-size: 15px;
  font-weight: 800;
  color: #fff;
  background: v-bind('themeVars.primaryColor');
  flex: none;
}

.app-sider {
  display: flex;
  flex-direction: column;
}

.sider-brand {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 16px 14px 15px;
  border-bottom: 1px solid v-bind('themeVars.borderColor');
}

.sider-brand.collapsed {
  justify-content: center;
  padding: 16px 0 15px;
}

.sider-title {
  font-size: 17px;
  font-weight: 700;
  white-space: nowrap;
}

.sider-menu {
  flex: 1;
  overflow-y: auto;
  padding-bottom: 8px;
}

.offline-strip {
  padding: 6px 16px;
  font-size: 13px;
  text-align: center;
}

.app-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 10px 28px;
  flex-wrap: wrap;
}

.header-title {
  min-width: 0;
}

.header-kicker {
  margin: 0 0 2px;
  font-size: 12px;
}

.header-h1 {
  margin: 0;
  font-size: 18px;
  font-weight: 700;
  line-height: 1.3;
}

.header-desc {
  display: block;
  margin-top: 2px;
  font-size: 12px;
}

.user-btn {
  padding-left: 6px;
}

.user-name {
  font-size: 14px;
}

.panel-region {
  outline: none;
  min-height: 100%;
}

/* 面板切换：轻量淡入，避免生硬跳变；偏好减少动态时关闭 */
.panel-fade-enter-active,
.panel-fade-leave-active {
  transition: opacity 0.13s ease;
}
.panel-fade-enter-from,
.panel-fade-leave-to {
  opacity: 0;
}
@media (prefers-reduced-motion: reduce) {
  .panel-fade-enter-active,
  .panel-fade-leave-active {
    transition: none;
  }
}

.muted {
  color: v-bind('themeVars.textColor3');
}
</style>
