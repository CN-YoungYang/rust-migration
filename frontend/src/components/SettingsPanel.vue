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

    <form v-else class="settings-form" :aria-busy="saving" @submit.prevent="saveSettings" @input="saved = false" @change="saved = false">
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
          <input id="settings-window-start" v-model="settings.windowStart" type="time" required :aria-invalid="invalidFields.windowStart" aria-describedby="settings-validation" />
        </div>
        <div class="form-group">
          <label for="settings-window-end">签到窗口结束</label>
          <input id="settings-window-end" v-model="settings.windowEnd" type="time" required :aria-invalid="invalidFields.windowEnd" aria-describedby="settings-validation" />
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
        <input id="settings-max-attempts" v-model.number="settings.maxAttemptsPerDay" type="number" min="1" max="100" :aria-invalid="invalidFields.maxAttemptsPerDay" aria-describedby="settings-validation" />
      </div>

      <div class="form-row">
        <div class="form-group">
          <label for="settings-delay-min">批量/定时签到最小延迟（秒）</label>
          <input id="settings-delay-min" v-model.number="settings.batchDelayMin" type="number" min="0" max="600" :aria-invalid="invalidFields.batchDelayMin" aria-describedby="settings-validation" />
        </div>
        <div class="form-group">
          <label for="settings-delay-max">批量/定时签到最大延迟（秒）</label>
          <input id="settings-delay-max" v-model.number="settings.batchDelayMax" type="number" min="0" max="600" :aria-invalid="invalidFields.batchDelayMax" aria-describedby="settings-validation" />
        </div>
      </div>

      <div class="form-group">
        <label for="settings-cleanup-latest">清理记录时保留最新条数</label>
        <input id="settings-cleanup-latest" v-model.number="settings.cleanupKeepLatest" type="number" min="0" max="10000" :aria-invalid="invalidFields.cleanupKeepLatest" aria-describedby="settings-validation" />
      </div>

      <p id="settings-validation" class="field-error-slot" :class="{ 'is-empty': validationErrors.length === 0 }" :role="validationErrors.length > 0 ? 'alert' : undefined">{{ validationErrors[0] || '\u00a0' }}</p>

    <button type="submit" class="btn-primary" :disabled="saving || validationErrors.length > 0" :data-state="saving ? 'loading' : (saved ? 'success' : undefined)">
        {{ saving ? '保存中...' : (saved ? '已保存' : '保存设置') }}
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
import { apiUrl, request, responseData } from '../utils/api'
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
const saved = ref(false)

const settingsStatusText = computed(() => {
  if (loading.value) return '正在加载设置...'
  if (loadError.value) return '设置加载失败'
  return settings.value.enabled ? '自动签到已启用' : '自动签到已停用'
})

const validationErrors = computed(() => {
  const errors: string[] = []
  if (minutesOf(settings.value.windowStart) === null) {
    errors.push('签到窗口开始时间格式无效。')
  }
  if (minutesOf(settings.value.windowEnd) === null) {
    errors.push('签到窗口结束时间格式无效。')
  }
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

const invalidFields = computed(() => ({
  windowStart: minutesOf(settings.value.windowStart) === null,
  windowEnd: minutesOf(settings.value.windowEnd) === null,
  maxAttemptsPerDay: settings.value.maxAttemptsPerDay < 1 || settings.value.maxAttemptsPerDay > 100,
  batchDelayMin: settings.value.batchDelayMin < 0 || settings.value.batchDelayMin > settings.value.batchDelayMax,
  batchDelayMax: settings.value.batchDelayMax < 0 || settings.value.batchDelayMax > 600 || settings.value.batchDelayMax < settings.value.batchDelayMin,
  cleanupKeepLatest: settings.value.cleanupKeepLatest < 0 || settings.value.cleanupKeepLatest > 10000,
}))

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
    const response = await request(apiUrl('/settings'))
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
    return
  }
  saving.value = true
  try {
    const response = await request(apiUrl('/settings'), {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(settings.value)
    })
    settings.value = await responseData<Settings>(response)
    saved.value = true
  } catch (error) {
    showToast(error instanceof Error ? error.message : '保存设置失败', 'error')
  } finally {
    saving.value = false
  }
}

onMounted(fetchSettings)
</script>

<style scoped>
.settings-panel { max-width: 860px; margin: 0 auto; padding: clamp(var(--space-sm), 2.5vw, var(--space-lg)) 0 var(--space-xl); }
.panel-header { display: flex; justify-content: space-between; align-items: flex-start; gap: var(--space-sm); margin-bottom: var(--space-md); flex-wrap: wrap; }
h2 { color: var(--text-strong); margin-bottom: var(--space-3xs); }
.panel-subtitle { color: var(--text-muted); font-size: var(--text-meta); }
h3 { color: var(--text-strong); margin-bottom: var(--space-sm); }
.status-strip { display: flex; gap: var(--space-2xs); flex-wrap: wrap; }
.status-pill { border-radius: var(--radius-pill); padding: var(--space-3xs) var(--space-xs); background: var(--border-strong); color: var(--text-faint); font-size: var(--text-meta); }
.status-pill.enabled { background: var(--success-soft); color: var(--color-success); }
.status-pill.disabled { background: var(--danger-soft); color: var(--color-danger); }
.settings-form { background: var(--bg-card); border: var(--rule-thin) solid var(--border); padding: var(--space-lg); border-radius: var(--radius-card); margin-bottom: var(--space-lg); box-shadow: var(--shadow-card); }
.form-row { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-sm); }
.form-group { margin-bottom: var(--space-md); }
.form-group label { display: block; color: var(--text); margin-bottom: var(--space-2xs); font-weight: 500; }
.form-group input[type="time"], .form-group input[type="number"] { width: 100%; padding: var(--space-2xs); background: var(--bg-well); border: var(--rule-thin) solid var(--border-input); border-radius: var(--radius-input); color: var(--text-strong); }
.switch { position: relative; display: inline-block; width: 50px; height: 24px; }
.switch input { opacity: 0; width: 0; height: 0; }
.slider { position: absolute; cursor: pointer; inset: 0; background-color: var(--color-rule-strong); transition: background-color var(--dur-short) var(--ease-out); border-radius: var(--radius-pill); }
.slider:before { position: absolute; content: ""; height: 18px; width: 18px; left: 3px; bottom: 3px; background-color: var(--color-paper); transition: transform var(--dur-short) var(--ease-out); border-radius: 50%; }
input:checked + .slider { background-color: var(--accent); }
input:checked + .slider:before { transform: translateX(26px); }
input:focus-visible + .slider { outline: 2px solid var(--focus-ring); outline-offset: 2px; }
.btn-primary { background: var(--accent); color: var(--color-accent-ink); border: none; padding: var(--space-xs) var(--space-md); border-radius: var(--radius-input); cursor: pointer; font-size: var(--text-md); font-weight: 600; }
.btn-primary:hover:not(:disabled) { background: var(--accent-hover); }
.btn-primary:disabled { opacity: 0.6; cursor: not-allowed; }
.validation-box { border: var(--rule-thin) solid var(--color-danger); background: var(--color-danger-soft); color: var(--color-danger); border-radius: var(--radius-card); padding: var(--space-xs) var(--space-sm); margin-bottom: var(--space-sm); display: grid; gap: var(--space-3xs); }
.load-error { grid-template-columns: minmax(0, 1fr) auto; align-items: center; }
.load-error button { border: var(--rule-thin) solid var(--border-strong); border-radius: var(--radius-input); background: var(--bg-elevated); color: var(--text-strong); padding: var(--space-2xs) var(--space-xs); }
.info-section { background: var(--bg-card); border: var(--rule-thin) solid var(--border); padding: var(--space-md); border-radius: var(--radius-card); }
.policy-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--space-sm); }
.policy-grid div { background: var(--bg-well); border: var(--rule-thin) solid var(--border); border-radius: var(--radius-card); padding: var(--space-sm); display: grid; gap: var(--space-3xs); }
.policy-grid span { color: var(--text-muted); font-size: var(--text-xs); }
.policy-grid strong { color: var(--text-strong); font-size: var(--text-sm); overflow-wrap: anywhere; }

@media (max-width: 768px) {
  .settings-panel { padding: var(--space-sm); }
  .form-row { grid-template-columns: 1fr; }
  .settings-form { padding: var(--space-sm); }
  .policy-grid { grid-template-columns: 1fr; }
}
</style>
