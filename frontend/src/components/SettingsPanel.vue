<template>
  <div class="settings-panel">
    <h2>全局设置</h2>


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

      <button type="submit" class="btn-primary" :disabled="saving">{{ saving ? '保存中...' : '保存设置' }}</button>
    </form>

    <div class="info-section">
      <h3>说明</h3>
      <ul>
        <li>后端会在签到窗口内执行自动签到。</li>
        <li>失败重试由全局设置和账户自身 retryEnabled 共同控制。</li>
        <li>每天最大尝试次数用于限制单个账户的自动签到尝试。</li>
        <li>批量与定时签到均为串行执行，账户间在最小/最大延迟区间内随机等待，并随机打乱顺序与 UA，降低被站点判定为批量签到的风险。</li>
      </ul>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
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
  updatedAt?: string
}

const settings = ref<Settings>({
  enabled: false,
  windowStart: '02:00',
  windowEnd: '05:00',
  retryEnabled: true,
  maxAttemptsPerDay: 3,
  batchDelayMin: 3,
  batchDelayMax: 10
})
const saving = ref(false)

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
.settings-panel { max-width: 800px; margin: 0 auto; padding: 2rem; }
h2 { color: #fff; margin-bottom: 2rem; }
h3 { color: #fff; margin-bottom: 1rem; }
.settings-form { background: #1a1a1a; padding: 2rem; border-radius: 8px; margin-bottom: 2rem; }
.form-row { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
.form-group { margin-bottom: 1.5rem; }
.form-group label { display: block; color: #ccc; margin-bottom: 0.5rem; font-weight: 500; }
.form-group input[type="time"], .form-group input[type="number"] { width: 100%; padding: 0.5rem; background: #2a2a2a; border: 1px solid #444; border-radius: 4px; color: #fff; }
.switch { position: relative; display: inline-block; width: 50px; height: 24px; }
.switch input { opacity: 0; width: 0; height: 0; }
.slider { position: absolute; cursor: pointer; inset: 0; background-color: #666; transition: 0.3s; border-radius: 24px; }
.slider:before { position: absolute; content: ""; height: 18px; width: 18px; left: 3px; bottom: 3px; background-color: white; transition: 0.3s; border-radius: 50%; }
input:checked + .slider { background-color: #0070f3; }
input:checked + .slider:before { transform: translateX(26px); }
.btn-primary { background: #0070f3; color: white; border: none; padding: 0.75rem 1.5rem; border-radius: 4px; cursor: pointer; font-size: 1rem; }
.btn-primary:hover { background: #0051cc; }
.info-section { background: #1a1a1a; padding: 1.5rem; border-radius: 8px; }
.info-section ul { color: #888; padding-left: 1.5rem; }
.info-section li { margin-bottom: 0.5rem; }

@media (max-width: 768px) {
  .settings-panel { padding: 1rem; }
  .form-row { grid-template-columns: 1fr; }
  .settings-form { padding: 1rem; }
}
</style>
