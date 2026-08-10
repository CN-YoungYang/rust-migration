<template>
  <div class="panel">
    <div class="panel-header">
      <div class="panel-heading">
        <h2 class="panel-title">全局设置</h2>
        <n-text depth="3" class="panel-subtitle">{{ settingsStatusText }}</n-text>
      </div>
      <n-space v-if="!loading && !loadError" align="center" :size="8" class="panel-tags">
        <n-tag round :bordered="false" :type="settings.enabled ? 'success' : 'default'">
          {{ settings.enabled ? '启用' : '停用' }}
        </n-tag>
        <n-tag round :bordered="false" type="info">{{ scheduleSummary }}</n-tag>
      </n-space>
    </div>

    <n-alert v-if="loadError" type="error" :show-icon="true" class="load-error" role="alert" :action="() => h(NButton, { size: 'small', onClick: fetchSettings }, { default: () => '重试' })">
      {{ loadError }}
    </n-alert>
    <div v-else-if="loading" class="state-hint" role="status" aria-live="polite">
      <n-spin size="large" />
      <p class="muted">正在加载设置…</p>
    </div>

    <n-form v-else class="settings-form" :model="settings" :disabled="saving" label-placement="left" label-width="180" @submit.prevent="saveSettings">
      <!-- 调度计划 -->
      <section class="field-group">
        <header class="field-group-head">
          <n-icon :component="TimeOutline" class="field-group-icon" aria-hidden="true" />
          <div class="field-group-titles">
            <h3 class="field-group-title">调度计划</h3>
            <p class="field-group-desc muted">标准 5 段 cron，命中任一表达式即触发一轮签到</p>
          </div>
        </header>
        <div class="field-group-body">
          <n-form-item label="启用自动签到" :show-feedback="false">
            <n-switch v-model:value="settings.enabled" @update:value="markDirty" />
          </n-form-item>
          <n-form-item label="触发时间" :show-feedback="false" :validation-status="invalidFields.scheduleCron ? 'error' : undefined">
            <div class="cron-list">
              <div v-for="(expr, index) in settings.scheduleCron" :key="index" class="cron-row">
                <n-input
                  v-model:value="settings.scheduleCron[index]"
                  placeholder="如 */5 2-5 * * *"
                  :status="validateCronExpr(expr) ? 'error' : undefined"
                  @update:value="markDirty"
                />
                <n-button size="small" quaternary circle :disabled="settings.scheduleCron.length <= 1" aria-label="删除该触发时间" @click="removeCronRow(index)">
                  <template #icon><n-icon :component="CloseOutline" /></template>
                </n-button>
              </div>
              <div class="cron-actions">
                <n-button size="small" dashed @click="addCronRow">
                  <template #icon><n-icon :component="AddOutline" /></template>
                  添加触发时间
                </n-button>
                <span class="cron-hint muted">语法：分 时 日 月 周，支持 * 、*/n 、a-b 、逗号列表</span>
              </div>
            </div>
          </n-form-item>
        </div>
      </section>

      <!-- 失败重试 -->
      <section class="field-group">
        <header class="field-group-head">
          <n-icon :component="RefreshOutline" class="field-group-icon" aria-hidden="true" />
          <div class="field-group-titles">
            <h3 class="field-group-title">失败重试</h3>
            <p class="field-group-desc muted">失败账户每日再试上限</p>
          </div>
        </header>
        <div class="field-group-body">
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
        </div>
      </section>

      <!-- 批量手动签到延迟 -->
      <section class="field-group">
        <header class="field-group-head">
          <n-icon :component="HandRightOutline" class="field-group-icon" aria-hidden="true" />
          <div class="field-group-titles">
            <h3 class="field-group-title">批量手动签到延迟</h3>
            <p class="field-group-desc muted">手动批量执行时账户间的随机等待</p>
          </div>
        </header>
        <div class="field-group-body">
          <n-form-item label="间隔区间（秒）" :show-feedback="false" :validation-status="invalidFields.batchDelayMin || invalidFields.batchDelayMax ? 'error' : undefined">
            <div class="delay-range">
              <n-input-number
                v-model:value="settings.batchDelayMin"
                :min="0"
                :max="600"
                :status="invalidFields.batchDelayMin ? 'error' : undefined"
                @update:value="markDirty"
                placeholder="最小"
              />
              <span class="delay-range-sep muted" aria-hidden="true">→</span>
              <n-input-number
                v-model:value="settings.batchDelayMax"
                :min="0"
                :max="600"
                :status="invalidFields.batchDelayMax ? 'error' : undefined"
                @update:value="markDirty"
                placeholder="最大"
              />
              <span class="delay-range-tag muted">{{ delaySummary }}</span>
            </div>
          </n-form-item>
        </div>
      </section>

      <!-- 定时签到延迟 -->
      <section class="field-group">
        <header class="field-group-head">
          <n-icon :component="TimerOutline" class="field-group-icon" aria-hidden="true" />
          <div class="field-group-titles">
            <h3 class="field-group-title">定时签到延迟</h3>
            <p class="field-group-desc muted">调度器串行执行账户间的随机等待</p>
          </div>
        </header>
        <div class="field-group-body">
          <n-form-item label="间隔区间（秒）" :show-feedback="false" :validation-status="invalidFields.scheduledDelayMin || invalidFields.scheduledDelayMax ? 'error' : undefined">
            <div class="delay-range">
              <n-input-number
                v-model:value="settings.scheduledDelayMin"
                :min="0"
                :max="600"
                :status="invalidFields.scheduledDelayMin ? 'error' : undefined"
                @update:value="markDirty"
                placeholder="最小"
              />
              <span class="delay-range-sep muted" aria-hidden="true">→</span>
              <n-input-number
                v-model:value="settings.scheduledDelayMax"
                :min="0"
                :max="600"
                :status="invalidFields.scheduledDelayMax ? 'error' : undefined"
                @update:value="markDirty"
                placeholder="最大"
              />
              <span class="delay-range-tag muted">{{ scheduledDelaySummary }}</span>
            </div>
          </n-form-item>
        </div>
      </section>

      <!-- 记录清理 -->
      <section class="field-group">
        <header class="field-group-head">
          <n-icon :component="TrashOutline" class="field-group-icon" aria-hidden="true" />
          <div class="field-group-titles">
            <h3 class="field-group-title">记录清理</h3>
            <p class="field-group-desc muted">自动清理保留的最近条数</p>
          </div>
        </header>
        <div class="field-group-body">
          <n-form-item label="保留最新条数" :show-feedback="false">
            <n-input-number
              v-model:value="settings.cleanupKeepLatest"
              :min="0"
              :max="10000"
              :status="invalidFields.cleanupKeepLatest ? 'error' : undefined"
              @update:value="markDirty"
            />
          </n-form-item>
        </div>
      </section>

      <n-alert v-if="validationErrors.length > 0" type="error" :show-icon="true" class="form-error" role="alert">
        {{ validationErrors[0] }}
      </n-alert>

      <div class="form-actions">
        <n-button
          type="primary"
          :loading="saving"
          :disabled="saving || validationErrors.length > 0"
          @click="saveSettings"
        >
          <template #icon v-if="saved"><n-icon :component="CheckmarkOutline" /></template>
          {{ saving ? '保存中…' : saved ? '已保存' : '保存设置' }}
        </n-button>
        <n-text v-if="dirty && !saved" depth="3" class="dirty-hint" role="status" aria-live="polite">
          <span class="dirty-dot" aria-hidden="true" /> 未保存
        </n-text>
      </div>
    </n-form>

    <div v-if="!loading && !loadError" class="info-section">
      <h3 class="section-title">当前执行策略</h3>
      <n-grid :cols="4" :x-gap="12" :y-gap="12" responsive="screen" item-responsive class="policy-grid">
        <n-grid-item>
          <n-card size="small" class="policy-card">
            <div class="policy-head">
              <n-icon :component="TimeOutline" class="policy-icon" aria-hidden="true" />
              <p class="policy-label">签到计划</p>
            </div>
            <p class="policy-value">{{ schedulePlanText }}</p>
          </n-card>
        </n-grid-item>
        <n-grid-item>
          <n-card size="small" class="policy-card">
            <div class="policy-head">
              <n-icon :component="RefreshOutline" class="policy-icon" aria-hidden="true" />
              <p class="policy-label">失败重试</p>
            </div>
            <p class="policy-value">{{ settings.retryEnabled ? `启用，最多 ${settings.maxAttemptsPerDay} 次/天` : '停用' }}</p>
          </n-card>
        </n-grid-item>
        <n-grid-item>
          <n-card size="small" class="policy-card">
            <div class="policy-head">
              <n-icon :component="HandRightOutline" class="policy-icon" aria-hidden="true" />
              <p class="policy-label">批量节奏</p>
            </div>
            <p class="policy-value">{{ delaySummary }}</p>
            <p class="policy-sub">定时：{{ scheduledDelaySummary }}</p>
          </n-card>
        </n-grid-item>
        <n-grid-item>
          <n-card size="small" class="policy-card">
            <div class="policy-head">
              <n-icon :component="TrashOutline" class="policy-icon" aria-hidden="true" />
              <p class="policy-label">记录清理</p>
            </div>
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
  NInput,
  NInputNumber,
  NSpace,
  NSpin,
  NSwitch,
  NTag,
  NText,
  useMessage,
  useThemeVars,
} from 'naive-ui'
import {
  AddOutline,
  CheckmarkOutline,
  CloseOutline,
  HandRightOutline,
  RefreshOutline,
  TimeOutline,
  TimerOutline,
  TrashOutline,
} from '@vicons/ionicons5'
import { apiUrl, request, responseData } from '../utils/api'
import { validateCronExpr } from '../utils/cron'

interface Settings {
  id?: string
  enabled: boolean
  scheduleCron: string[]
  retryEnabled: boolean
  maxAttemptsPerDay: number
  batchDelayMin: number
  batchDelayMax: number
  scheduledDelayMin: number
  scheduledDelayMax: number
  cleanupKeepLatest: number
  updatedAt?: string
}

const message = useMessage()
const themeVars = useThemeVars()
const settings = ref<Settings>({
  enabled: false,
  scheduleCron: ['*/5 2-5 * * *'],
  retryEnabled: true,
  maxAttemptsPerDay: 3,
  batchDelayMin: 3,
  batchDelayMax: 10,
  scheduledDelayMin: 3,
  scheduledDelayMax: 10,
  cleanupKeepLatest: 500,
})
const loading = ref(true)
const loadError = ref('')
const saving = ref(false)
const saved = ref(false)
// 远端最后一次持久化的快照，用于判“本地是否相对已保存值有改动”
const persisted = ref<Settings | null>(null)

const settingsStatusText = computed(() => {
  if (loading.value) return '正在加载设置…'
  if (loadError.value) return '设置加载失败'
  return settings.value.enabled ? '自动签到已启用' : '自动签到已停用'
})

const validationErrors = computed(() => {
  const errors: string[] = []
  const cronList = settings.value.scheduleCron
  if (cronList.length === 0 || cronList.every((e) => !e.trim())) {
    errors.push('至少配置一个触发时间。')
  }
  if (cronList.length > 20) {
    errors.push('触发时间最多 20 个。')
  }
  for (const expr of cronList) {
    const err = validateCronExpr(expr)
    if (err) {
      errors.push(`触发时间「${expr.trim() || '(空)'}」${err}。`)
      break
    }
  }
  if (settings.value.maxAttemptsPerDay < 1 || settings.value.maxAttemptsPerDay > 100) {
    errors.push('每天最大尝试次数必须在 1 到 100 之间。')
  }
  if (settings.value.batchDelayMin < 0 || settings.value.batchDelayMax < 0) {
    errors.push('批量延迟不能小于 0 秒。')
  }
  if (settings.value.batchDelayMin > settings.value.batchDelayMax) {
    errors.push('批量最小延迟不能大于最大延迟。')
  }
  if (settings.value.batchDelayMax > 600) {
    errors.push('批量最大延迟不能超过 600 秒。')
  }
  if (settings.value.scheduledDelayMin < 0 || settings.value.scheduledDelayMax < 0) {
    errors.push('定时延迟不能小于 0 秒。')
  }
  if (settings.value.scheduledDelayMin > settings.value.scheduledDelayMax) {
    errors.push('定时最小延迟不能大于最大延迟。')
  }
  if (settings.value.scheduledDelayMax > 600) {
    errors.push('定时最大延迟不能超过 600 秒。')
  }
  if (settings.value.cleanupKeepLatest < 0 || settings.value.cleanupKeepLatest > 10000) {
    errors.push('清理保留条数必须在 0 到 10000 之间。')
  }
  return errors
})

const invalidFields = computed(() => ({
  scheduleCron:
    settings.value.scheduleCron.length === 0 ||
    settings.value.scheduleCron.some((e) => validateCronExpr(e) !== null),
  maxAttemptsPerDay: settings.value.maxAttemptsPerDay < 1 || settings.value.maxAttemptsPerDay > 100,
  batchDelayMin: settings.value.batchDelayMin < 0 || settings.value.batchDelayMin > settings.value.batchDelayMax,
  batchDelayMax: settings.value.batchDelayMax < 0 || settings.value.batchDelayMax > 600 || settings.value.batchDelayMax < settings.value.batchDelayMin,
  scheduledDelayMin: settings.value.scheduledDelayMin < 0 || settings.value.scheduledDelayMin > settings.value.scheduledDelayMax,
  scheduledDelayMax: settings.value.scheduledDelayMax < 0 || settings.value.scheduledDelayMax > 600 || settings.value.scheduledDelayMax < settings.value.scheduledDelayMin,
  cleanupKeepLatest: settings.value.cleanupKeepLatest < 0 || settings.value.cleanupKeepLatest > 10000,
}))

function summarizeDelay(min: number, max: number): string {
  if (min === 0 && max === 0) {
    return '不等待'
  }
  if (min === max) {
    return `${min} 秒固定`
  }
  return `${min}–${max} 秒随机`
}

const delaySummary = computed(() =>
  summarizeDelay(settings.value.batchDelayMin, settings.value.batchDelayMax),
)
const scheduledDelaySummary = computed(() =>
  summarizeDelay(settings.value.scheduledDelayMin, settings.value.scheduledDelayMax),
)

// 面板头部/政策卡展示的调度计划摘要
const scheduleSummary = computed(() => {
  const list = settings.value.scheduleCron.filter((e) => e.trim() !== '')
  if (list.length === 0) return '未配置触发时间'
  return list.length === 1 ? list[0] : `${list[0]} 等 ${list.length} 个`
})

const schedulePlanText = computed(() => {
  if (!settings.value.enabled) return '自动签到未启用'
  const list = settings.value.scheduleCron.filter((e) => e.trim() !== '')
  if (list.length === 0) return '未配置触发时间'
  return list.join(' · ')
})

const addCronRow = () => {
  if (settings.value.scheduleCron.length >= 20) return
  settings.value.scheduleCron.push('')
  markDirty()
}

const removeCronRow = (index: number) => {
  if (settings.value.scheduleCron.length <= 1) return
  settings.value.scheduleCron.splice(index, 1)
  markDirty()
}

// dirty：本地相对远端最近一次保存值有改动；保存成功后重新对齐快照
const dirty = computed(() => {
  const p = persisted.value
  if (!p) return true
  const s = settings.value
  return (
    s.enabled !== p.enabled ||
    s.scheduleCron.join('\n') !== p.scheduleCron.join('\n') ||
    s.retryEnabled !== p.retryEnabled ||
    s.maxAttemptsPerDay !== p.maxAttemptsPerDay ||
    s.batchDelayMin !== p.batchDelayMin ||
    s.batchDelayMax !== p.batchDelayMax ||
    s.scheduledDelayMin !== p.scheduledDelayMin ||
    s.scheduledDelayMax !== p.scheduledDelayMax ||
    s.cleanupKeepLatest !== p.cleanupKeepLatest
  )
})

function markDirty() {
  saved.value = false
}

const fetchSettings = async () => {
  loading.value = true
  loadError.value = ''
  try {
    const response = await request(apiUrl('/settings'))
    const data = await responseData<Settings>(response)
    settings.value = data
    persisted.value = { ...data }
    saved.value = false
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
    const data = await responseData<Settings>(response)
    settings.value = data
    persisted.value = { ...data }
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
  margin-bottom: 18px;
}

.panel-heading {
  min-width: 0;
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

.panel-tags {
  flex-wrap: wrap;
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

.settings-form {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

/* 职能分组：左侧图标 + 标题 + 副标题，右侧表单控件 */
.field-group {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 16px 16px 18px;
  border: 1px solid v-bind('themeVars.dividerColor');
  border-radius: 10px;
  background: v-bind('themeVars.cardColor');
}

.field-group-head {
  display: flex;
  align-items: flex-start;
  gap: 10px;
}

.field-group-icon {
  flex: none;
  margin-top: 1px;
  font-size: 18px;
  color: v-bind('themeVars.primaryColor');
}

.field-group-titles {
  min-width: 0;
}

.field-group-title {
  margin: 0;
  font-size: 14px;
  font-weight: 700;
  line-height: 1.4;
}

.field-group-desc {
  margin: 2px 0 0;
  font-size: 12px;
}

.field-group-body {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

/* 标签宽度收紧到分组卡内更紧凑，且留出右侧行内控件空间 */
.settings-form :deep(.n-form-item) {
  grid-template-columns: 180px 1fr;
}
.settings-form :deep(.n-form-item .n-form-item-label) {
  padding-right: 12px;
}

/* cron 触发时间列表：每行输入框 + 删除按钮，下方添加按钮与语法提示 */
.cron-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  width: 100%;
  max-width: 520px;
}

.cron-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.cron-row :deep(.n-input) {
  flex: 1;
  min-width: 0;
}

.cron-actions {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.cron-hint {
  font-size: 12px;
}

/* 延迟区间：min / max / 速览文案一行 */
.delay-range {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.delay-range-sep {
  font-size: 14px;
  line-height: 1;
}

.delay-range-tag {
  margin-left: 4px;
  font-size: 12px;
  white-space: nowrap;
}

.delay-range :deep(.n-input-number) {
  width: 96px;
}

.form-error {
  margin-top: 4px;
}

.form-actions {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
  margin-top: 4px;
}

.dirty-hint {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
}

.dirty-dot {
  display: inline-block;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: v-bind('themeVars.warningColor');
}

.info-section {
  margin-top: 28px;
}

.section-title {
  margin: 0 0 12px;
  font-size: 15px;
  font-weight: 700;
}

.policy-grid {
  margin-top: 12px;
}

.policy-card {
  height: 100%;
}

.policy-head {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 6px;
}

.policy-icon {
  font-size: 15px;
  color: v-bind('themeVars.primaryColor');
}

.policy-label {
  margin: 0;
  font-size: 12px;
  color: v-bind('themeVars.textColor3');
}

.policy-value {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  word-break: break-word;
}

.policy-sub {
  margin: 4px 0 0;
  font-size: 12px;
  color: v-bind('themeVars.textColor3');
}

.muted {
  color: v-bind('themeVars.textColor3');
}

/* 窄屏：分组卡内表单退回单列，标签置于控件上方，区间元素换行 */
@media (max-width: 720px) {
  .settings-form :deep(.n-form-item) {
    grid-template-columns: 1fr;
  }
  .settings-form :deep(.n-form-item .n-form-item-label) {
    padding: 0 0 4px;
  }
  .delay-range :deep(.n-input-number) {
    width: 100%;
  }
}
</style>
