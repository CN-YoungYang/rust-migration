<template>
  <div class="panel">
    <div class="panel-header">
      <div>
        <h2 class="panel-title">全局设置</h2>
        <n-text depth="3" class="panel-subtitle">{{ settingsStatusText }}</n-text>
      </div>
      <n-space v-if="!loading && !loadError" align="center" :size="8">
        <n-tag round :bordered="false" :type="settings.enabled ? 'success' : 'default'">
          {{ settings.enabled ? '启用' : '停用' }}
        </n-tag>
        <n-tag round :bordered="false" type="info">{{ settings.windowStart }} - {{ settings.windowEnd }}</n-tag>
      </n-space>
    </div>

    <n-alert v-if="loadError" type="error" :show-icon="true" class="load-error" role="alert" :action="() => h(NButton, { size: 'small', onClick: fetchSettings }, { default: () => '重试' })">
      {{ loadError }}
    </n-alert>
    <div v-else-if="loading" class="state-hint" role="status" aria-live="polite">
      <n-spin size="large" />
      <p class="muted">正在加载设置…</p>
    </div>

    <n-form v-else class="settings-form" :model="settings" :disabled="saving" label-placement="left" label-width="220" @submit.prevent="saveSettings">
      <n-form-item label="启用自动签到" :show-feedback="false">
        <n-switch v-model:value="settings.enabled" @update:value="markDirty" />
      </n-form-item>

      <n-grid :cols="2" :x-gap="16" responsive="screen" item-responsive>
        <n-grid-item>
          <n-form-item label="签到窗口开始" :show-feedback="false" :validation-status="invalidFields.windowStart ? 'error' : undefined">
            <n-time-picker v-model:value="windowStartPicker" format="HH:mm" clearable />
          </n-form-item>
        </n-grid-item>
        <n-grid-item>
          <n-form-item label="签到窗口结束" :show-feedback="false" :validation-status="invalidFields.windowEnd ? 'error' : undefined">
            <n-time-picker v-model:value="windowEndPicker" format="HH:mm" clearable />
          </n-form-item>
        </n-grid-item>
      </n-grid>

      <n-form-item label="启用失败重试" :show-feedback="false">
        <n-switch v-model:value="settings.retryEnabled" @update:value="markDirty" />
      </n-form-item>

      <n-form-item label="每天最大尝试次数" :show-feedback="false">
        <n-input-number
          v-model:value="settings.maxAttemptsPerDay"
          :min="1"
          :max="100"
          :status="invalidFields.maxAttemptsPerDay ? 'error' : undefined"
          @update:value="markDirty"
        />
      </n-form-item>

      <n-grid :cols="2" :x-gap="16" responsive="screen" item-responsive>
        <n-grid-item>
          <n-form-item label="批量/定时签到最小延迟（秒）" :show-feedback="false">
            <n-input-number
              v-model:value="settings.batchDelayMin"
              :min="0"
              :max="600"
              :status="invalidFields.batchDelayMin ? 'error' : undefined"
              @update:value="markDirty"
              style="width: 100%"
            />
          </n-form-item>
        </n-grid-item>
        <n-grid-item>
          <n-form-item label="批量/定时签到最大延迟（秒）" :show-feedback="false">
            <n-input-number
              v-model:value="settings.batchDelayMax"
              :min="0"
              :max="600"
              :status="invalidFields.batchDelayMax ? 'error' : undefined"
              @update:value="markDirty"
              style="width: 100%"
            />
          </n-form-item>
        </n-grid-item>
      </n-grid>

      <n-form-item label="清理记录时保留最新条数" :show-feedback="false">
        <n-input-number
          v-model:value="settings.cleanupKeepLatest"
          :min="0"
          :max="10000"
          :status="invalidFields.cleanupKeepLatest ? 'error' : undefined"
          @update:value="markDirty"
        />
      </n-form-item>

      <n-alert v-if="validationErrors.length > 0" type="error" :show-icon="true" class="form-error" role="alert">
        {{ validationErrors[0] }}
      </n-alert>

      <n-button
        type="primary"
        :loading="saving"
        :disabled="saving || validationErrors.length > 0"
        @click="saveSettings"
      >
        <template #icon v-if="saved"><n-icon :component="CheckmarkOutline" /></template>
        {{ saving ? '保存中…' : saved ? '已保存' : '保存设置' }}
      </n-button>
    </n-form>

    <div v-if="!loading && !loadError" class="info-section">
      <h3 class="section-title">当前执行策略</h3>
      <n-grid :cols="4" :x-gap="12" :y-gap="12" responsive="screen" item-responsive class="policy-grid">
        <n-grid-item>
          <n-card size="small">
            <p class="policy-label">签到窗口</p>
            <p class="policy-value">{{ nextWindowText }}</p>
          </n-card>
        </n-grid-item>
        <n-grid-item>
          <n-card size="small">
            <p class="policy-label">失败重试</p>
            <p class="policy-value">{{ settings.retryEnabled ? `启用，最多 ${settings.maxAttemptsPerDay} 次/天` : '停用' }}</p>
          </n-card>
        </n-grid-item>
        <n-grid-item>
          <n-card size="small">
            <p class="policy-label">批量节奏</p>
            <p class="policy-value">{{ delaySummary }}</p>
          </n-card>
        </n-grid-item>
        <n-grid-item>
          <n-card size="small">
            <p class="policy-label">记录清理</p>
            <p class="policy-value">保留最新 {{ settings.cleanupKeepLatest }} 条</p>
          </n-card>
        </n-grid-item>
      </n-grid>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, h, ref, onMounted } from 'vue'
import {
  NAlert,
  NButton,
  NCard,
  NForm,
  NFormItem,
  NGrid,
  NGridItem,
  NIcon,
  NInputNumber,
  NSpace,
  NSpin,
  NSwitch,
  NTag,
  NText,
  NTimePicker,
  useMessage,
  useThemeVars,
} from 'naive-ui'
import { CheckmarkOutline } from '@vicons/ionicons5'
import { apiUrl, request, responseData } from '../utils/api'

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

const message = useMessage()
const themeVars = useThemeVars()
const settings = ref<Settings>({
  enabled: false,
  windowStart: '02:00',
  windowEnd: '05:00',
  retryEnabled: true,
  maxAttemptsPerDay: 3,
  batchDelayMin: 3,
  batchDelayMax: 10,
  cleanupKeepLatest: 500,
})
const loading = ref(true)
const loadError = ref('')
const saving = ref(false)
const saved = ref(false)

const settingsStatusText = computed(() => {
  if (loading.value) return '正在加载设置…'
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

// NTimePicker 用时间戳作为值，这里与设置的 "HH:MM" 字符串互转
function timeToTimestamp(value: string): number | null {
  const minutes = minutesOf(value)
  if (minutes === null) return null
  const date = new Date()
  date.setHours(Math.floor(minutes / 60), minutes % 60, 0, 0)
  return date.getTime()
}

function timestampToTime(timestamp: number): string {
  const date = new Date(timestamp)
  return `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`
}

const windowStartPicker = computed<number | null>({
  get: () => timeToTimestamp(settings.value.windowStart),
  set: (value) => {
    settings.value.windowStart = value !== null ? timestampToTime(value) : ''
    saved.value = false
  },
})

const windowEndPicker = computed<number | null>({
  get: () => timeToTimestamp(settings.value.windowEnd),
  set: (value) => {
    settings.value.windowEnd = value !== null ? timestampToTime(value) : ''
    saved.value = false
  },
})

function markDirty() {
  saved.value = false
}

const fetchSettings = async () => {
  loading.value = true
  loadError.value = ''
  try {
    const response = await request(apiUrl('/settings'))
    settings.value = await responseData<Settings>(response)
  } catch (error) {
    loadError.value = error instanceof Error ? error.message : '加载设置失败'
    message.error(loadError.value)
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
      body: JSON.stringify(settings.value),
    })
    settings.value = await responseData<Settings>(response)
    saved.value = true
  } catch (error) {
    message.error(error instanceof Error ? error.message : '保存设置失败')
  } finally {
    saving.value = false
  }
}

onMounted(fetchSettings)
</script>

<style scoped>
.panel-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
  margin-bottom: 14px;
}

.panel-title {
  margin: 0;
  font-size: 18px;
  font-weight: 700;
  line-height: 1.3;
}

.panel-subtitle {
  display: block;
  margin-top: 2px;
  font-size: 13px;
}

.load-error {
  margin-bottom: 12px;
}

.state-hint {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  padding: 40px 0;
}

.form-error {
  margin-bottom: 14px;
}

.info-section {
  margin-top: 24px;
}

.section-title {
  margin: 0 0 12px;
  font-size: 15px;
  font-weight: 700;
}

.policy-grid {
  margin-top: 12px;
}

.policy-label {
  margin: 0 0 4px;
  font-size: 12px;
  color: v-bind('themeVars.textColor3');
}

.policy-value {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
}

.muted {
  color: v-bind('themeVars.textColor3');
}
</style>
