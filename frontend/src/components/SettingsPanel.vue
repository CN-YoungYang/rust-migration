<template>
  <div class="settings-panel">
    <div class="panel-header">
      <div>
        <h2>全局设置</h2>
        <p class="panel-subtitle">{{ settings.enabled ? '自动签到已启用' : '自动签到已停用' }}</p>
      </div>
      <div class="status-strip">
        <span :class="['status-pill', settings.enabled ? 'enabled' : 'disabled']">
          {{ settings.enabled ? '启用' : '停用' }}
        </span>
        <span class="status-pill">{{ settings.windowStart }} - {{ settings.windowEnd }}</span>
      </div>
    </div>


    <form @submit.prevent="saveSettings" class="settings-form">
      <div class="form-group">
        <label>启用自动签到</label>
        <label class="switch">
          <input v-model="settings.enabled" type="checkbox" />
          <span class="slider"></span>
        </label>
      </div>

      <div class="form-row">
        <div class="form-group">
          <label>签到窗口开始</label>
          <input v-model="settings.windowStart" type="time" required />
        </div>
        <div class="form-group">
          <label>签到窗口结束</label>
          <input v-model="settings.windowEnd" type="time" required />
        </div>
      </div>

      <div class="form-group">
        <label>启用失败重试</label>
        <label class="switch">
          <input v-model="settings.retryEnabled" type="checkbox" />
          <span class="slider"></span>
        </label>
      </div>

      <div class="form-group">
        <label>每天最大尝试次数</label>
        <input v-model.number="settings.maxAttemptsPerDay" type="number" min="1" max="100" />
      </div>

      <div class="form-row">
        <div class="form-group">
          <label>批量/定时签到最小延迟（秒）</label>
          <input v-model.number="settings.batchDelayMin" type="number" min="0" max="600" />
        </div>
        <div class="form-group">
          <label>批量/定时签到最大延迟（秒）</label>
          <input v-model.number="settings.batchDelayMax" type="number" min="0" max="600" />
        </div>
      </div>

      <div class="form-group">
        <label>清理记录时保留最新条数</label>
        <input v-model.number="settings.cleanupKeepLatest" type="number" min="0" max="10000" />
      </div>

      <div v-if="validationErrors.length > 0" class="validation-box">
        <p v-for="error in validationErrors" :key="error">{{ error }}</p>
      </div>

      <button type="submit" class="btn-primary" :disabled="saving || validationErrors.length > 0">
        {{ saving ? '保存中...' : '保存设置' }}
      </button>
    </form>

    <div class="info-section">
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
const saving = ref(false)

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
  try {
    const response = await request(apiUrl('/settings'), {
      headers: authHeaders()
    })
    settings.value = await responseData<Settings>(response)
  } catch (error) {
    showToast(error instanceof Error ? error.message : '加载设置失败', 'error')
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
.settings-panel { max-width: 860px; margin: 0 auto; padding: 2rem; }
.panel-header { display: flex; justify-content: space-between; align-items: flex-start; gap: 1rem; margin-bottom: 1.5rem; flex-wrap: wrap; }
h2 { color: #fff; margin-bottom: 0.25rem; }
.panel-subtitle { color: #94a3b8; font-size: 0.9rem; }
h3 { color: #fff; margin-bottom: 1rem; }
.status-strip { display: flex; gap: 0.5rem; flex-wrap: wrap; }
.status-pill { border-radius: 999px; padding: 0.3rem 0.65rem; background: #334155; color: #cbd5e1; font-size: 0.8rem; }
.status-pill.enabled { background: rgba(16, 185, 129, 0.18); color: #34d399; }
.status-pill.disabled { background: rgba(239, 68, 68, 0.18); color: #f87171; }
.settings-form { background: #111827; border: 1px solid #263241; padding: 2rem; border-radius: 8px; margin-bottom: 2rem; box-shadow: 0 14px 35px rgba(0, 0, 0, 0.18); }
.form-row { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
.form-group { margin-bottom: 1.5rem; }
.form-group label { display: block; color: #d1d5db; margin-bottom: 0.5rem; font-weight: 500; }
.form-group input[type="time"], .form-group input[type="number"] { width: 100%; padding: 0.6rem; background: #0b1220; border: 1px solid #374151; border-radius: 6px; color: #fff; }
.switch { position: relative; display: inline-block; width: 50px; height: 24px; }
.switch input { opacity: 0; width: 0; height: 0; }
.slider { position: absolute; cursor: pointer; inset: 0; background-color: #475569; transition: 0.3s; border-radius: 24px; }
.slider:before { position: absolute; content: ""; height: 18px; width: 18px; left: 3px; bottom: 3px; background-color: white; transition: 0.3s; border-radius: 50%; }
input:checked + .slider { background-color: #2563eb; }
input:checked + .slider:before { transform: translateX(26px); }
.btn-primary { background: #2563eb; color: white; border: none; padding: 0.75rem 1.5rem; border-radius: 6px; cursor: pointer; font-size: 1rem; font-weight: 600; }
.btn-primary:hover:not(:disabled) { background: #1d4ed8; }
.btn-primary:disabled { opacity: 0.6; cursor: not-allowed; }
.validation-box { border: 1px solid rgba(239, 68, 68, 0.45); background: rgba(239, 68, 68, 0.08); color: #fca5a5; border-radius: 8px; padding: 0.85rem 1rem; margin-bottom: 1rem; display: grid; gap: 0.35rem; }
.info-section { background: #111827; border: 1px solid #263241; padding: 1.5rem; border-radius: 8px; }
.policy-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 1rem; }
.policy-grid div { background: #0b1220; border: 1px solid #263241; border-radius: 8px; padding: 0.9rem; display: grid; gap: 0.35rem; }
.policy-grid span { color: #94a3b8; font-size: 0.85rem; }
.policy-grid strong { color: #f8fafc; font-size: 0.95rem; overflow-wrap: anywhere; }

@media (max-width: 768px) {
  .settings-panel { padding: 1rem; }
  .form-row { grid-template-columns: 1fr; }
  .settings-form { padding: 1rem; }
  .policy-grid { grid-template-columns: 1fr; }
}
</style>
