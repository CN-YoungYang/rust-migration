<template>
  <section class="notification-panel">
    <div class="panel-header">
      <div>
        <h2>通知设置</h2>
        <p class="panel-subtitle">已配置 {{ configs.length }} 个，启用 {{ enabledCount }} 个</p>
      </div>
      <button class="primary" @click="startCreate" :disabled="saving || loading">新建通知</button>
    </div>

    <form v-if="editing" class="notification-form" :aria-busy="saving" aria-labelledby="notification-form-title" @submit.prevent="saveConfig">
      <h3 id="notification-form-title">{{ form.id ? '编辑通知' : '新建通知' }}</h3>
      <div class="form-row">
        <label>
          通知类型
          <select v-model="form.notifyType" :disabled="Boolean(form.id)" :aria-invalid="invalidFields.notifyType" aria-describedby="notification-validation">
            <option value="webhook">Webhook</option>
            <option value="telegram">Telegram</option>
            <option value="email">邮件</option>
          </select>
        </label>
        <label class="switch-row">
          启用
          <input v-model="form.enabled" type="checkbox" />
        </label>
      </div>

      <div class="form-row">
        <label class="switch-row">
          签到失败通知
          <input v-model="form.onFailure" type="checkbox" />
        </label>
        <label>
          连续失败阈值
          <input v-model.number="form.failureThreshold" type="number" min="1" max="100" :aria-invalid="invalidFields.failureThreshold" aria-describedby="notification-validation" />
        </label>
      </div>

      <div class="form-row">
        <label class="switch-row">
          余额过低通知
          <input v-model="form.onBalanceLow" type="checkbox" />
        </label>
        <label>
          余额阈值（美元）
          <input v-model.number="form.balanceThreshold" type="number" min="0" step="0.01" :aria-invalid="invalidFields.balanceThreshold" aria-describedby="notification-validation" />
        </label>
      </div>

      <template v-if="form.notifyType === 'webhook'">
        <label>
          Webhook URL
          <input v-model.trim="form.webhookUrl" type="url" :aria-invalid="invalidFields.webhookUrl" aria-describedby="notification-validation" />
        </label>
        <div class="form-row">
          <label>
            HTTP 方法
            <select v-model="form.webhookMethod">
              <option value="POST">POST</option>
              <option value="PUT">PUT</option>
            </select>
          </label>
          <label>
            Headers JSON
            <input v-model.trim="form.webhookHeaders" placeholder='{"X-Token":"..."}' :aria-invalid="invalidFields.webhookHeaders" aria-describedby="notification-validation" />
          </label>
        </div>
      </template>

      <template v-if="form.notifyType === 'telegram'">
        <label>
          Bot Token
          <input v-model.trim="form.telegramBotToken" type="password" autocomplete="new-password" :aria-invalid="invalidFields.telegramBotToken" aria-describedby="notification-validation" />
        </label>
        <label>
          Chat ID
          <input v-model.trim="form.telegramChatId" :aria-invalid="invalidFields.telegramChatId" aria-describedby="notification-validation" />
        </label>
      </template>

      <template v-if="form.notifyType === 'email'">
        <div class="form-row">
          <label>
            SMTP 主机
            <input v-model.trim="form.emailSmtpHost" :aria-invalid="invalidFields.emailSmtpHost" aria-describedby="notification-validation" />
          </label>
          <label>
            SMTP 端口
            <input v-model.number="form.emailSmtpPort" type="number" min="1" max="65535" :aria-invalid="invalidFields.emailSmtpPort" aria-describedby="notification-validation" />
          </label>
        </div>
        <label>
          SMTP 用户名
          <input v-model.trim="form.emailSmtpUser" :aria-invalid="invalidFields.emailSmtpUser" aria-describedby="notification-validation" />
        </label>
        <label>
          SMTP 密码
          <input v-model.trim="form.emailSmtpPassword" type="password" autocomplete="new-password" :aria-invalid="invalidFields.emailSmtpPassword" aria-describedby="notification-validation" />
        </label>
        <div class="form-row">
          <label>
            发件人
            <input v-model.trim="form.emailFrom" type="email" :aria-invalid="invalidFields.emailFrom" aria-describedby="notification-validation" />
          </label>
          <label>
            收件人
            <input v-model.trim="form.emailTo" :aria-invalid="invalidFields.emailTo" aria-describedby="notification-validation" />
          </label>
        </div>
      </template>

      <label>
        备注
        <input v-model.trim="form.note" />
      </label>

      <div class="preview-grid">
        <div>
          <span>触发条件</span>
          <strong>{{ formTriggerSummary }}</strong>
        </div>
        <div>
          <span>发送目标</span>
          <strong>{{ formTargetSummary }}</strong>
        </div>
      </div>

      <p id="notification-validation" class="field-error-slot" :class="{ 'is-empty': validationErrors.length === 0 }" :role="validationErrors.length > 0 ? 'alert' : undefined">{{ validationErrors[0] || '\u00a0' }}</p>

      <div class="form-actions">
        <button type="submit" class="primary" :disabled="saving || validationErrors.length > 0" :data-state="saving ? 'loading' : undefined">
          {{ saving ? '保存中...' : '保存' }}
        </button>
        <button type="button" @click="cancelEdit" :disabled="saving">取消</button>
      </div>
    </form>

    <div v-if="loading" class="empty" role="status" aria-live="polite">加载中...</div>
    <div v-else-if="configs.length === 0" class="empty" role="status">暂无通知配置，可使用“新建通知”添加。</div>

    <div v-else class="notification-list" :aria-busy="loading">
      <article v-for="config in configs" :key="config.id" class="notification-card">
        <div class="config-main">
          <div class="title-row">
            <strong>{{ typeLabel(config.notifyType) }}</strong>
            <span class="badge" :class="{ disabled: !config.enabled }">{{ config.enabled ? '启用' : '停用' }}</span>
          </div>
          <p class="muted">
            {{ triggerSummary(config) }}
          </p>
          <p class="muted">{{ targetSummary(config) }}</p>
          <p v-if="testResults[config.id]" :class="['test-result', testResults[config.id].success ? 'success' : 'failed']" role="status" aria-live="polite">
            {{ testResults[config.id].message }} · {{ testResults[config.id].testedAt }}
          </p>
          <p v-if="config.note" class="note">{{ config.note }}</p>
        </div>
        <div class="actions">
          <button @click="testConfig(config)" :disabled="Boolean(testingId) || saving">
            {{ testingId === config.id ? '测试中...' : '测试' }}
          </button>
          <button @click="startEdit(config)" :disabled="saving || Boolean(testingId)">编辑</button>
          <button class="danger" @click="deleteConfig(config.id)" :disabled="saving || Boolean(testingId)">删除</button>
        </div>
      </article>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { apiUrl, request, responseData } from '../utils/api'
import { confirmAction, showToast } from '../utils/toast'

interface NotificationConfig {
  id: string
  notifyType: 'email' | 'webhook' | 'telegram'
  enabled: boolean
  onFailure: boolean
  failureThreshold: number
  onBalanceLow: boolean
  balanceThreshold: number | null
  emailSmtpHost?: string | null
  emailSmtpPort?: number | null
  emailSmtpUser?: string | null
  emailFrom?: string | null
  emailTo?: string | null
  webhookUrl?: string | null
  webhookMethod?: string | null
  webhookHeaders?: string | null
  telegramChatId?: string | null
  note?: string | null
  createdAt?: string
  updatedAt?: string
}

interface NotificationForm extends Partial<NotificationConfig> {
  emailSmtpPassword?: string
  telegramBotToken?: string
}

const configs = ref<NotificationConfig[]>([])
const loading = ref(false)
const saving = ref(false)
const testingId = ref('')
const editing = ref(false)
const form = ref<NotificationForm>(emptyForm())
const testResults = ref<Record<string, { success: boolean; message: string; testedAt: string }>>({})

const enabledCount = computed(() => configs.value.filter((config) => config.enabled).length)

const validationErrors = computed(() => {
  const errors: string[] = []
  if (!form.value.notifyType) errors.push('请选择通知类型。')
  if ((form.value.failureThreshold ?? 1) < 1 || (form.value.failureThreshold ?? 1) > 100) {
    errors.push('连续失败阈值必须在 1 到 100 之间。')
  }
  if (form.value.onBalanceLow && (form.value.balanceThreshold ?? -1) < 0) {
    errors.push('余额阈值不能小于 0。')
  }

  if (form.value.notifyType === 'webhook') {
    if (!form.value.webhookUrl?.trim()) errors.push('Webhook URL 不能为空。')
    if (form.value.webhookUrl && !isHttpUrl(form.value.webhookUrl)) {
      errors.push('Webhook URL 必须是 http 或 https 地址。')
    }
    if (form.value.webhookHeaders?.trim()) {
      const headerError = validateHeadersJson(form.value.webhookHeaders)
      if (headerError) errors.push(headerError)
    }
  }

  if (form.value.notifyType === 'telegram') {
    if (!form.value.id && !form.value.telegramBotToken?.trim()) {
      errors.push('新建 Telegram 通知时必须填写 Bot Token。')
    }
    if (!form.value.telegramChatId?.trim()) errors.push('Telegram Chat ID 不能为空。')
  }

  if (form.value.notifyType === 'email') {
    if (!form.value.emailSmtpHost?.trim()) errors.push('SMTP 主机不能为空。')
    const port = form.value.emailSmtpPort ?? 0
    if (port < 1 || port > 65535) errors.push('SMTP 端口必须在 1 到 65535 之间。')
    if (!form.value.emailSmtpUser?.trim()) errors.push('SMTP 用户名不能为空。')
    if (!form.value.id && !form.value.emailSmtpPassword?.trim()) {
      errors.push('新建邮件通知时必须填写 SMTP 密码。')
    }
    if (!form.value.emailFrom?.trim()) errors.push('发件人不能为空。')
    if (!form.value.emailTo?.trim()) errors.push('收件人不能为空。')
  }

  return errors
})

const invalidFields = computed(() => {
  const value = form.value
  const port = value.emailSmtpPort ?? 0
  return {
    notifyType: !value.notifyType,
    failureThreshold: (value.failureThreshold ?? 1) < 1 || (value.failureThreshold ?? 1) > 100,
    balanceThreshold: value.onBalanceLow && (value.balanceThreshold ?? -1) < 0,
    webhookUrl: value.notifyType === 'webhook' && (!value.webhookUrl?.trim() || !isHttpUrl(value.webhookUrl)),
    webhookHeaders: value.notifyType === 'webhook' && Boolean(value.webhookHeaders?.trim() && validateHeadersJson(value.webhookHeaders)),
    telegramBotToken: value.notifyType === 'telegram' && !value.id && !value.telegramBotToken?.trim(),
    telegramChatId: value.notifyType === 'telegram' && !value.telegramChatId?.trim(),
    emailSmtpHost: value.notifyType === 'email' && !value.emailSmtpHost?.trim(),
    emailSmtpPort: value.notifyType === 'email' && (port < 1 || port > 65535),
    emailSmtpUser: value.notifyType === 'email' && !value.emailSmtpUser?.trim(),
    emailSmtpPassword: value.notifyType === 'email' && !value.id && !value.emailSmtpPassword?.trim(),
    emailFrom: value.notifyType === 'email' && !value.emailFrom?.trim(),
    emailTo: value.notifyType === 'email' && !value.emailTo?.trim(),
  }
})

const formTriggerSummary = computed(() => {
  const parts: string[] = []
  if (form.value.onFailure) parts.push(`失败 ${form.value.failureThreshold ?? 1} 次`)
  if (form.value.onBalanceLow) parts.push(`余额低于 $${form.value.balanceThreshold ?? 0}`)
  return parts.length > 0 ? parts.join('，') : '未启用触发条件'
})

const formTargetSummary = computed(() => {
  if (form.value.notifyType === 'email') return form.value.emailTo || '邮件收件人未填写'
  if (form.value.notifyType === 'telegram') return form.value.telegramChatId || 'Telegram Chat ID 未填写'
  return form.value.webhookUrl || 'Webhook URL 未填写'
})

function emptyForm(): NotificationForm {
  return {
    notifyType: 'webhook',
    enabled: true,
    onFailure: true,
    failureThreshold: 1,
    onBalanceLow: false,
    balanceThreshold: null,
    webhookMethod: 'POST'
  }
}

function typeLabel(type: string): string {
  return type === 'email' ? '邮件' : type === 'telegram' ? 'Telegram' : 'Webhook'
}

function isHttpUrl(value: string): boolean {
  try {
    const url = new URL(value)
    return url.protocol === 'http:' || url.protocol === 'https:'
  } catch {
    return false
  }
}

function validateHeadersJson(value: string): string {
  try {
    const parsed = JSON.parse(value)
    if (!parsed || Array.isArray(parsed) || typeof parsed !== 'object') {
      return 'Headers JSON 必须是对象。'
    }
    for (const [key, headerValue] of Object.entries(parsed)) {
      if (!key.trim()) return 'Header 名称不能为空。'
      if (typeof headerValue !== 'string') return `Header ${key} 的值必须是字符串。`
    }
    return ''
  } catch {
    return 'Headers JSON 格式无效。'
  }
}

function triggerSummary(config: NotificationConfig): string {
  const parts: string[] = []
  if (config.onFailure) parts.push(`失败连续 ${config.failureThreshold} 次`)
  if (config.onBalanceLow) parts.push(`余额低于 $${config.balanceThreshold ?? 0}`)
  return parts.length > 0 ? parts.join('，') : '未启用触发条件'
}

function targetSummary(config: NotificationConfig): string {
  if (config.notifyType === 'email') return `发送至 ${config.emailTo || '-'}`
  if (config.notifyType === 'telegram') return `Chat ID ${config.telegramChatId || '-'}`
  return config.webhookUrl || '-'
}

function startCreate() {
  form.value = emptyForm()
  editing.value = true
}

function startEdit(config: NotificationConfig) {
  form.value = { ...config, emailSmtpPassword: '', telegramBotToken: '' }
  editing.value = true
}

function cancelEdit() {
  editing.value = false
  form.value = emptyForm()
}

function buildPayload() {
  const raw = { ...form.value }
  if (!raw.emailSmtpPassword) delete raw.emailSmtpPassword
  if (!raw.telegramBotToken) delete raw.telegramBotToken
  if (!raw.onBalanceLow) raw.balanceThreshold = null
  if (raw.webhookHeaders !== undefined && !raw.webhookHeaders?.trim()) raw.webhookHeaders = null
  if (raw.note !== undefined && !raw.note?.trim()) raw.note = null
  delete raw.id
  return raw
}

async function loadConfigs() {
  loading.value = true
  try {
    const response = await request(apiUrl('/notifications'))
    configs.value = await responseData<NotificationConfig[]>(response)
  } catch (error) {
    showToast(error instanceof Error ? error.message : '加载通知配置失败', 'error')
  } finally {
    loading.value = false
  }
}

async function saveConfig() {
  if (validationErrors.value.length > 0) {
    return
  }
  saving.value = true
  try {
    const id = form.value.id
    await request(apiUrl(id ? `/notifications/${id}` : '/notifications'), {
      method: id ? 'PUT' : 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(buildPayload())
    })
    cancelEdit()
    await loadConfigs()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '保存通知配置失败', 'error')
  } finally {
    saving.value = false
  }
}

async function testConfig(config: NotificationConfig) {
  testingId.value = config.id
  try {
    const response = await request(apiUrl(`/notifications/${config.id}/test`), {
      method: 'POST'
    })
    const result = await responseData<{ success: boolean; message?: string }>(response)
    const message = result.message || '测试完成'
    testResults.value = {
      ...testResults.value,
      [config.id]: {
        success: result.success,
        message,
        testedAt: new Date().toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
      }
    }
    if (!result.success) showToast(message, 'error')
  } catch (error) {
    const message = error instanceof Error ? error.message : '测试通知失败'
    testResults.value = {
      ...testResults.value,
      [config.id]: {
        success: false,
        message,
        testedAt: new Date().toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
      }
    }
    showToast(message, 'error')
  } finally {
    testingId.value = ''
  }
}

async function deleteConfig(id: string) {
  if (!(await confirmAction('确定要删除此通知配置吗？'))) return
  try {
    await request(apiUrl(`/notifications/${id}`), {
      method: 'DELETE'
    })
    await loadConfigs()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '删除通知配置失败', 'error')
  }
}

onMounted(loadConfigs)
</script>

<style scoped>
/* design-system: design.md · Workbench panel · NotificationPanel */
.notification-panel { max-width: 1000px; margin: 0 auto; padding: clamp(var(--space-sm), 2.5vw, var(--space-lg)) 0 var(--space-xl); }
.panel-header { display: flex; justify-content: space-between; align-items: flex-start; gap: var(--space-sm); margin-bottom: var(--space-md); }
h2, h3 { color: var(--text-strong); }
.panel-subtitle { color: var(--text-muted); font-size: var(--text-meta); margin-top: var(--space-3xs); }
.notification-form,
.notification-card {
  background: var(--bg-card);
  border: var(--rule-thin) solid var(--border);
  border-radius: var(--radius-card);
  padding: var(--space-md);
}
.notification-form { display: grid; gap: var(--space-sm); margin-bottom: var(--space-md); }
.form-row { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-sm); }
label { color: var(--text); display: grid; gap: var(--space-2xs); }
.switch-row { align-content: center; grid-template-columns: auto 1fr; align-items: center; }
input,
select {
  color: var(--text-strong);
  background: var(--bg-well);
  border: var(--rule-thin) solid var(--border-input);
  border-radius: var(--radius-input);
  padding: var(--space-2xs);
}
input:focus-visible,
select:focus-visible {
  outline: 2px solid var(--focus-ring);
  outline-offset: 1px;
}
button:focus-visible {
  outline: 2px solid var(--focus-ring);
  outline-offset: 2px;
}
.form-actions,
.actions { display: flex; gap: var(--space-xs); align-items: center; }
.preview-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--space-xs);
}
.preview-grid div {
  background: var(--bg-well);
  border: var(--rule-thin) solid var(--border);
  border-radius: var(--radius-card);
  padding: var(--space-xs);
  display: grid;
  gap: var(--space-3xs);
}
.preview-grid span {
  color: var(--text-muted);
  font-size: var(--text-xs);
}
.preview-grid strong {
  color: var(--text-strong);
  font-size: var(--text-sm);
  overflow-wrap: anywhere;
}
.validation-box {
  border: var(--rule-thin) solid var(--color-danger);
  background: var(--color-danger-soft);
  color: var(--color-danger);
  border-radius: var(--radius-card);
  padding: var(--space-xs) var(--space-sm);
  display: grid;
  gap: var(--space-3xs);
}
.notification-list { display: grid; gap: var(--space-sm); }
.notification-card { display: flex; justify-content: space-between; gap: var(--space-sm); transition: border-color var(--dur-short) var(--ease-out), background-color var(--dur-short) var(--ease-out); }
.notification-card:hover { background: var(--bg-elevated); border-color: var(--border-strong); }
.config-main { min-width: 0; }
.title-row { display: flex; align-items: center; gap: var(--space-2xs); margin-bottom: var(--space-2xs); }
.badge { background: var(--success-soft); color: var(--color-success); border-radius: var(--radius-pill); padding: var(--space-3xs) var(--space-2xs); font-size: var(--text-xs); }
.badge.disabled { background: var(--color-paper-3); color: var(--color-muted); }
.muted { color: var(--text-muted); margin: var(--space-3xs) 0; overflow-wrap: anywhere; }
.note { color: var(--color-warning); margin: var(--space-3xs) 0; }
.test-result { margin-top: var(--space-2xs); font-size: var(--text-xs); }
.test-result.success { color: var(--color-success); }
.test-result.failed { color: var(--color-danger); }
button {
  color: var(--text-strong);
  background: var(--border-input);
  border: 0;
  border-radius: var(--radius-input);
  padding: var(--space-2xs) var(--space-xs);
}
button:disabled { opacity: 0.6; cursor: not-allowed; }
button.primary,
.primary { background: var(--accent); color: var(--color-accent-ink); }
button:hover:not(:disabled) { background: var(--color-paper-2); }
button.primary:hover:not(:disabled),
.primary:hover:not(:disabled) { background: var(--accent-hover); }
button.danger { background: var(--color-danger-soft); color: var(--color-danger); }
button.danger:hover:not(:disabled) { background: var(--color-danger-soft); }
.empty { color: var(--text-muted); text-align: center; padding: var(--space-lg); }

@media (max-width: 47.99rem) {
  .notification-panel { padding: var(--space-sm); }
  .panel-header { display: grid; }
  .panel-header button { width: 100%; }
  .notification-form { grid-template-columns: 1fr; }
  .form-row { display: flex; flex-direction: column; gap: var(--space-sm); }
  .preview-grid { grid-template-columns: 1fr; }
  .notification-card { display: grid; grid-template-columns: 1fr; }
  .actions { flex-wrap: wrap; }
  .actions button,
  .panel-header button { width: 100%; }
}
</style>
