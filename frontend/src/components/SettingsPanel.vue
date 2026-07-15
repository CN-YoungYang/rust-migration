<template>
  <div class="settings-panel">
    <div class="panel-header">
      <div>
        <h2>全局设置</h2>
        <p class="panel-subtitle">{{ settingsStatusText }}</p>
      </div>
      <div v-if="!loading && !loadError" class="status-strip">
        <span :class="['status-pill', settings.enabled ? 'enabled' : 'disabled']">
          {{ settings.enabled ? '启用' : '停用' }}
        </span>
        <span class="status-pill">{{ settings.windowStart }} - {{ settings.windowEnd }}</span>
      </div>
    </div>


    <div v-if="loadError" class="validation-box load-error" role="alert">
      <span>{{ loadError }}</span>
      <button type="button" @click="fetchSettings">重试</button>
    </div>
    <p v-else-if="loading" class="loading-state" role="status" aria-live="polite">正在加载设置...</p>

    <form v-else class="settings-form" :aria-busy="saving" @submit.prevent="saveSettings">
      <div class="form-group">
        <label for="settings-enabled">启用自动签到</label>
        <label class="switch">
          <input id="settings-enabled" v-model="settings.enabled" type="checkbox" />
          <span class="slider" aria-hidden="true"></span>
        </label>
      </div>

      <div class="form-row">
        <div class="form-group">
          <label for="settings-window-start">签到窗口开始</label>
          <input id="settings-window-start" v-model="settings.windowStart" type="time" required />
        </div>
        <div class="form-group">
          <label for="settings-window-end">签到窗口结束</label>
          <input id="settings-window-end" v-model="settings.windowEnd" type="time" required />
        </div>
      </div>

      <div class="form-group">
        <label for="settings-retry-enabled">启用失败重试</label>
        <label class="switch">
          <input id="settings-retry-enabled" v-model="settings.retryEnabled" type="checkbox" />
          <span class="slider" aria-hidden="true"></span>
        </label>
      </div>

      <div class="form-group">
        <label for="settings-max-attempts">每天最大尝试次数</label>
        <input id="settings-max-attempts" v-model.number="settings.maxAttemptsPerDay" type="number" min="1" max="100" />
      </div>

      <div class="form-row">
        <div class="form-group">
          <label for="settings-delay-min">批量/定时签到最小延迟（秒）</label>
          <input id="settings-delay-min" v-model.number="settings.batchDelayMin" type="number" min="0" max="600" />
        </div>
        <div class="form-group">
          <label for="settings-delay-max">批量/定时签到最大延迟（秒）</label>
          <input id="settings-delay-max" v-model.number="settings.batchDelayMax" type="number" min="0" max="600" />
        </div>
      </div>

      <div class="form-group">
        <label for="settings-cleanup-latest">清理记录时保留最新条数</label>
        <input id="settings-cleanup-latest" v-model.number="settings.cleanupKeepLatest" type="number" min="0" max="10000" />
      </div>

      <div v-if="validationErrors.length > 0" class="validation-box" role="alert">
        <p v-for="error in validationErrors" :key="error">{{ error }}</p>
      </div>

      <button type="submit" class="btn-primary" :disabled="saving || validationErrors.length > 0">
        {{ saving ? '保存中...' : '保存设置' }}
      </button>
    </form>

    <div v-if="!loading && !loadError" class="info-section">
      <h3>当前执行策略</h3>
      <div class="policy-grid">
        <div>
          <span>签到窗口</span>
          <strong>{{ nextWindowText }}</strong>
        </div>
        <div>
          <span>失败重试</span>
          <strong>{{ settings.retryEnabled ? `启用，最多 ${settings.maxAttemptsPerDay} 次/天` : '停用' }}</strong>
        </div>
        <div>
          <span>批量节奏</span>
          <strong>{{ delaySummary }}</strong>
        </div>
        <div>
          <span>记录清理</span>
          <strong>保留最新 {{ settings.cleanupKeepLatest }} 条</strong>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted } from 'vue'
import { apiUrl, authHeaders, request, responseData } from '../utils/api'
import { showToast } from '../utils/toast'

interface Settings {
  id?: string
  enabled: boolean
  windowStart: string
  windowEnd: string
  retryEnabled: boolean
  maxAttemptsPerDay: number
  batchDelayMin: number
  batchDelayMax: number
  cleanupKeepLatest: number
  updatedAt?: string
}

const settings = ref<Settings>({
  enabled: false,
  windowStart: '02:00',
  windowEnd: '05:00',
  retryEnabled: true,
  maxAttemptsPerDay: 3,
  batchDelayMin: 3,
  batchDelayMax: 10,
  cleanupKeepLatest: 500
})
const loading = ref(true)
const loadError = ref('')
const saving = ref(false)

const settingsStatusText = computed(() => {
  if (loading.value) return '正在加载设置...'
  if (loadError.value) return '设置加载失败'
  return settings.value.enabled ? '自动签到已启用' : '自动签到已停用'
})

const validationErrors = computed(() => {
  const errors: string[] = []
  if (settings.value.maxAttemptsPerDay < 1 || settings.value.maxAttemptsPerDay > 100) {
    errors.push('每天最大尝试次数必须在 1 到 100 之间。')
  }
  if (settings.value.batchDelayMin < 0 || settings.value.batchDelayMax < 0) {
    errors.push('批量延迟不能小于 0 秒。')
  }
  if (settings.value.batchDelayMin > settings.value.batchDelayMax) {
    errors.push('最小延迟不能大于最大延迟。')
  }
  if (settings.value.batchDelayMax > 600) {
    errors.push('最大延迟不能超过 600 秒。')
  }
  if (settings.value.cleanupKeepLatest < 0 || settings.value.cleanupKeepLatest > 10000) {
    errors.push('清理保留条数必须在 0 到 10000 之间。')
  }
  return errors
})

const delaySummary = computed(() => {
  if (settings.value.batchDelayMin === 0 && settings.value.batchDelayMax === 0) {
    return '不等待'
  }
  if (settings.value.batchDelayMin === settings.value.batchDelayMax) {
    return `${settings.value.batchDelayMin} 秒固定间隔`
  }
  return `${settings.value.batchDelayMin} 到 ${settings.value.batchDelayMax} 秒随机间隔`
})

function minutesOf(value: string): number | null {
  const [hour, minute] = value.split(':').map(Number)
  if (!Number.isInteger(hour) || !Number.isInteger(minute)) return null
  if (hour < 0 || hour > 23 || minute < 0 || minute > 59) return null
  return hour * 60 + minute
}

const nextWindowText = computed(() => {
  if (!settings.value.enabled) return '自动签到未启用'
  const start = minutesOf(settings.value.windowStart)
  const end = minutesOf(settings.value.windowEnd)
  if (start === null || end === null) return '时间格式无效'

  const now = new Date()
  const current = now.getHours() * 60 + now.getMinutes()
  const range = `${settings.value.windowStart} - ${settings.value.windowEnd}`

  if (start <= end) {
    if (current >= start && current <= end) return `当前窗口内，${range}`
    if (current < start) return `今日 ${range}`
    return `明日 ${range}`
  }

  if (current >= start || current <= end) return `当前跨日窗口内，${range}`
  return `今日 ${range}`
})

const fetchSettings = async () => {
  loading.value = true
  loadError.value = ''
  try {
    const response = await request(apiUrl('/settings'), {
      headers: authHeaders()
    })
    settings.value = await responseData<Settings>(response)
  } catch (error) {
    loadError.value = error instanceof Error ? error.message : '加载设置失败'
    showToast(loadError.value, 'error')
  } finally {
    loading.value = false
  }
}

const saveSettings = async () => {
  if (validationErrors.value.length > 0) {
    showToast(validationErrors.value[0], 'error')
    return
  }
  saving.value = true
  try {
    const response = await request(apiUrl('/settings'), {
      method: 'PUT',
      headers: { ...authHeaders(), 'Content-Type': 'application/json' },
      body: JSON.stringify(settings.value)
    })
    settings.value = await responseData<Settings>(response)
    showToast('设置已保存', 'success')
  } catch (error) {
    showToast(error instanceof Error ? error.message : '保存设置失败', 'error')
  } finally {
    saving.value = false
  }
}

onMounted(fetchSettings)
</script>

<style scoped>
.settings-panel { max-width: 860px; margin: 0 auto; padding: clamp(1rem, 2.5vw, 2.25rem) 0 3rem; }
.panel-header { display: flex; justify-content: space-between; align-items: flex-start; gap: 1rem; margin-bottom: 1.5rem; flex-wrap: wrap; }
h2 { color: var(--text-strong); margin-bottom: 0.25rem; }
.panel-subtitle { color: var(--text-muted); font-size: 0.9rem; }
h3 { color: var(--text-strong); margin-bottom: 1rem; }
.status-strip { display: flex; gap: 0.5rem; flex-wrap: wrap; }
.status-pill { border-radius: var(--radius-pill); padding: 0.3rem 0.65rem; background: var(--border-strong); color: var(--text-faint); font-size: 0.8rem; }
.status-pill.enabled { background: var(--success-soft); color: #34d399; }
.status-pill.disabled { background: var(--danger-soft); color: #f87171; }
.settings-form { background: var(--bg-card); border: 1px solid var(--border); padding: 2rem; border-radius: var(--radius); margin-bottom: 2rem; box-shadow: var(--shadow-card); }
.form-row { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
.form-group { margin-bottom: 1.5rem; }
.form-group label { display: block; color: var(--text); margin-bottom: 0.5rem; font-weight: 500; }
.form-group input[type="time"], .form-group input[type="number"] { width: 100%; padding: 0.6rem; background: var(--bg-well); border: 1px solid var(--border-input); border-radius: 6px; color: var(--text-strong); }
.switch { position: relative; display: inline-block; width: 50px; height: 24px; }
.switch input { opacity: 0; width: 0; height: 0; }
.slider { position: absolute; cursor: pointer; inset: 0; background-color: #475569; transition: 0.3s; border-radius: 24px; }
.slider:before { position: absolute; content: ""; height: 18px; width: 18px; left: 3px; bottom: 3px; background-color: white; transition: 0.3s; border-radius: 50%; }
input:checked + .slider { background-color: var(--accent); }
input:checked + .slider:before { transform: translateX(26px); }
input:focus-visible + .slider { outline: 2px solid var(--focus-ring); outline-offset: 2px; }
.btn-primary { background: var(--accent); color: white; border: none; padding: 0.75rem 1.5rem; border-radius: 6px; cursor: pointer; font-size: 1rem; font-weight: 600; }
.btn-primary:hover:not(:disabled) { background: var(--accent-hover); }
.btn-primary:disabled { opacity: 0.6; cursor: not-allowed; }
.validation-box { border: 1px solid rgba(239, 68, 68, 0.45); background: rgba(239, 68, 68, 0.08); color: #fca5a5; border-radius: var(--radius); padding: 0.85rem 1rem; margin-bottom: 1rem; display: grid; gap: 0.35rem; }
.load-error { grid-template-columns: minmax(0, 1fr) auto; align-items: center; }
.load-error button { border: 1px solid var(--border-strong); border-radius: 6px; background: var(--bg-elevated); color: var(--text-strong); padding: 0.45rem 0.8rem; }
.info-section { background: var(--bg-card); border: 1px solid var(--border); padding: 1.5rem; border-radius: var(--radius); }
.policy-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 1rem; }
.policy-grid div { background: var(--bg-well); border: 1px solid var(--border); border-radius: var(--radius); padding: 0.9rem; display: grid; gap: 0.35rem; }
.policy-grid span { color: var(--text-muted); font-size: 0.85rem; }
.policy-grid strong { color: var(--text-strong); font-size: 0.95rem; overflow-wrap: anywhere; }

@media (max-width: 768px) {
  .settings-panel { padding: 1rem; }
  .form-row { grid-template-columns: 1fr; }
  .settings-form { padding: 1rem; }
  .policy-grid { grid-template-columns: 1fr; }
}
</style>
