<template>
  <div class="panel">
    <div class="panel-header">
      <div>
        <h2 class="panel-title">通知设置</h2>
        <n-text depth="3" class="panel-subtitle">已配置 {{ configs.length }} 个，启用 {{ enabledCount }} 个</n-text>
      </div>
      <n-button type="primary" size="small" :disabled="saving || loading" @click="startCreate">新建通知</n-button>
    </div>

    <!-- 编辑 / 新建表单 -->
    <n-card v-if="editing" class="notification-form" :bordered="true" aria-labelledby="notification-form-title">
      <template #header>
        <h3 id="notification-form-title" class="form-title">{{ form.id ? '编辑通知' : '新建通知' }}</h3>
      </template>
      <n-form :model="form" label-placement="top" :disabled="saving">
        <n-grid :cols="2" :x-gap="16" responsive="screen" item-responsive>
          <n-grid-item>
            <n-form-item label="通知类型" :show-feedback="false">
              <n-select
                v-model:value="form.notifyType"
                :options="typeOptions"
                :disabled="Boolean(form.id)"
                :status="invalidFields.notifyType ? 'error' : undefined"
              />
            </n-form-item>
          </n-grid-item>
          <n-grid-item>
            <n-form-item label="启用" :show-feedback="false">
              <n-switch v-model:value="form.enabled" />
            </n-form-item>
          </n-grid-item>
          <n-grid-item>
            <n-form-item label="签到失败通知" :show-feedback="false">
              <n-switch v-model:value="form.onFailure" />
            </n-form-item>
          </n-grid-item>
          <n-grid-item>
            <n-form-item label="连续失败阈值" :show-feedback="false">
              <n-input-number
                v-model:value="form.failureThreshold"
                :min="1"
                :max="100"
                :status="invalidFields.failureThreshold ? 'error' : undefined"
                style="width: 100%"
              />
            </n-form-item>
          </n-grid-item>
          <n-grid-item>
            <n-form-item label="余额过低通知" :show-feedback="false">
              <n-switch v-model:value="form.onBalanceLow" />
            </n-form-item>
          </n-grid-item>
          <n-grid-item>
            <n-form-item label="余额阈值（美元）" :show-feedback="false">
              <n-input-number
                v-model:value="form.balanceThreshold"
                :min="0"
                :step="0.01"
                placeholder="0"
                :status="invalidFields.balanceThreshold ? 'error' : undefined"
                style="width: 100%"
              />
            </n-form-item>
          </n-grid-item>
        </n-grid>

        <template v-if="form.notifyType === 'webhook'">
          <n-form-item label="Webhook URL" :show-feedback="false">
            <n-input
              v-model:value="form.webhookUrl"
              type="text"
              placeholder="https://example.com/hook"
              :status="invalidFields.webhookUrl ? 'error' : undefined"
            />
          </n-form-item>
          <n-grid :cols="2" :x-gap="16" responsive="screen" item-responsive>
            <n-grid-item>
              <n-form-item label="HTTP 方法" :show-feedback="false">
                <n-select
                  v-model:value="form.webhookMethod"
                  :options="[{ label: 'POST', value: 'POST' }, { label: 'PUT', value: 'PUT' }]"
                />
              </n-form-item>
            </n-grid-item>
            <n-grid-item>
              <n-form-item label="Headers JSON" :show-feedback="false">
                <n-input
                  v-model:value="form.webhookHeaders"
                  type="text"
                  placeholder='{"X-Token":"..."}'
                  :status="invalidFields.webhookHeaders ? 'error' : undefined"
                />
              </n-form-item>
            </n-grid-item>
          </n-grid>
        </template>

        <template v-if="form.notifyType === 'telegram'">
          <n-form-item label="Bot Token" :show-feedback="false">
            <n-input
              v-model:value="form.telegramBotToken"
              type="password"
              show-password-on="click"
              autocomplete="new-password"
              placeholder="留空保持不变"
              :disabled="clearTelegramToken"
              :status="invalidFields.telegramBotToken ? 'error' : undefined"
            />
          </n-form-item>
          <n-form-item v-if="form.id" :show-feedback="false" class="clear-option">
            <n-checkbox v-model:checked="clearTelegramToken">清除已保存的 Bot Token</n-checkbox>
          </n-form-item>
          <n-form-item label="Chat ID" :show-feedback="false">
            <n-input
              v-model:value="form.telegramChatId"
              :status="invalidFields.telegramChatId ? 'error' : undefined"
            />
          </n-form-item>
        </template>

        <template v-if="form.notifyType === 'email'">
          <n-grid :cols="2" :x-gap="16" responsive="screen" item-responsive>
            <n-grid-item>
              <n-form-item label="SMTP 主机" :show-feedback="false">
                <n-input
                  v-model:value="form.emailSmtpHost"
                  :status="invalidFields.emailSmtpHost ? 'error' : undefined"
                />
              </n-form-item>
            </n-grid-item>
            <n-grid-item>
              <n-form-item label="SMTP 端口" :show-feedback="false">
                <n-input-number
                  v-model:value="form.emailSmtpPort"
                  :min="1"
                  :max="65535"
                  :status="invalidFields.emailSmtpPort ? 'error' : undefined"
                  style="width: 100%"
                />
              </n-form-item>
            </n-grid-item>
          </n-grid>
          <n-form-item label="SMTP 用户名" :show-feedback="false">
            <n-input
              v-model:value="form.emailSmtpUser"
              :status="invalidFields.emailSmtpUser ? 'error' : undefined"
            />
          </n-form-item>
          <n-form-item label="SMTP 密码" :show-feedback="false">
            <n-input
              v-model:value="form.emailSmtpPassword"
              type="password"
              show-password-on="click"
              autocomplete="new-password"
              placeholder="留空保持不变"
              :disabled="clearSmtpPassword"
              :status="invalidFields.emailSmtpPassword ? 'error' : undefined"
            />
          </n-form-item>
          <n-form-item v-if="form.id" :show-feedback="false" class="clear-option">
            <n-checkbox v-model:checked="clearSmtpPassword">清除已保存的 SMTP 密码</n-checkbox>
          </n-form-item>
          <n-grid :cols="2" :x-gap="16" responsive="screen" item-responsive>
            <n-grid-item>
              <n-form-item label="发件人" :show-feedback="false">
                <n-input
                  v-model:value="form.emailFrom"
                  placeholder="sender@example.com"
                  :status="invalidFields.emailFrom ? 'error' : undefined"
                />
              </n-form-item>
            </n-grid-item>
            <n-grid-item>
              <n-form-item label="收件人" :show-feedback="false">
                <n-input
                  v-model:value="form.emailTo"
                  :status="invalidFields.emailTo ? 'error' : undefined"
                />
              </n-form-item>
            </n-grid-item>
          </n-grid>
        </template>

        <n-form-item label="备注" :show-feedback="false">
          <n-input v-model:value="form.note" />
        </n-form-item>

        <n-descriptions :column="2" size="small" bordered class="preview-grid">
          <n-descriptions-item label="触发条件">{{ formTriggerSummary }}</n-descriptions-item>
          <n-descriptions-item label="发送目标">{{ formTargetSummary }}</n-descriptions-item>
        </n-descriptions>

        <n-alert v-if="validationErrors.length > 0" type="error" :show-icon="true" class="form-error" role="alert">
          {{ validationErrors[0] }}
        </n-alert>

        <n-space :size="8" class="form-actions">
          <n-button
            type="primary"
            :loading="saving"
            :disabled="saving || validationErrors.length > 0"
            @click="saveConfig"
          >
            {{ saving ? '保存中…' : '保存' }}
          </n-button>
          <n-button :disabled="saving" @click="cancelEdit">取消</n-button>
        </n-space>
      </n-form>
    </n-card>

    <div v-if="loading" class="state-hint" role="status" aria-live="polite">
      <n-spin size="large" />
      <p class="muted">加载中…</p>
    </div>
    <n-empty v-else-if="configs.length === 0" description="暂无通知配置，可使用「新建通知」添加。" />

    <div v-else class="notification-list" :aria-busy="loading">
      <n-card v-for="config in configs" :key="config.id" class="notification-card" size="small" :bordered="true">
        <div class="config-main">
          <div class="title-row">
            <strong>{{ typeLabel(config.notifyType) }}</strong>
            <n-tag size="small" :bordered="false" :type="config.enabled ? 'success' : 'default'">
              {{ config.enabled ? '启用' : '停用' }}
            </n-tag>
          </div>
          <p class="muted">{{ triggerSummary(config) }}</p>
          <p class="muted">{{ targetSummary(config) }}</p>
          <n-alert
            v-if="testResults[config.id]"
            :type="testResults[config.id].success ? 'success' : 'error'"
            size="small"
            :show-icon="true"
            class="test-result"
            role="status"
            aria-live="polite"
          >
            {{ testResults[config.id].message }} · {{ testResults[config.id].testedAt }}
          </n-alert>
          <p v-if="config.note" class="note">{{ config.note }}</p>
        </div>
        <template #footer>
          <n-space :size="8">
            <n-button
              size="small"
              secondary
              :loading="testingId === config.id"
              :disabled="Boolean(testingId) || saving"
              @click="testConfig(config)"
            >
              {{ testingId === config.id ? '测试中…' : '测试' }}
            </n-button>
            <n-button size="small" :disabled="saving || Boolean(testingId)" @click="startEdit(config)">编辑</n-button>
            <n-popconfirm @positive-click="deleteConfig(config.id)">
              <template #trigger>
                <n-button size="small" tertiary type="error" :disabled="saving || Boolean(testingId)">删除</n-button>
              </template>
              确定要删除此通知配置吗？此操作不可撤销。
            </n-popconfirm>
          </n-space>
        </template>
      </n-card>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import {
  NAlert,
  NButton,
  NCard,
  NCheckbox,
  NDescriptions,
  NDescriptionsItem,
  NEmpty,
  NForm,
  NFormItem,
  NGrid,
  NGridItem,
  NInput,
  NInputNumber,
  NPopconfirm,
  NSelect,
  NSpace,
  NSpin,
  NSwitch,
  NTag,
  NText,
  useMessage,
  useThemeVars,
} from 'naive-ui'
import { apiUrl, request, responseData } from '../utils/api'

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
  emailSmtpPassword?: string | null
  telegramBotToken?: string | null
}

const message = useMessage()
const themeVars = useThemeVars()
const configs = ref<NotificationConfig[]>([])
const loading = ref(false)
const saving = ref(false)
const testingId = ref('')
const editing = ref(false)
const form = ref<NotificationForm>(emptyForm())
// 编辑已有配置时，勾选后发送 null 清空已加密保存的凭证（后端三态清空）
const clearSmtpPassword = ref(false)
const clearTelegramToken = ref(false)
const testResults = ref<Record<string, { success: boolean; message: string; testedAt: string }>>({})

const typeOptions = [
  { label: 'Webhook', value: 'webhook' },
  { label: 'Telegram', value: 'telegram' },
  { label: '邮件', value: 'email' },
]

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
    webhookMethod: 'POST',
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
  clearSmtpPassword.value = false
  clearTelegramToken.value = false
  editing.value = true
}

function startEdit(config: NotificationConfig) {
  form.value = { ...config, emailSmtpPassword: '', telegramBotToken: '' }
  clearSmtpPassword.value = false
  clearTelegramToken.value = false
  editing.value = true
}

function cancelEdit() {
  editing.value = false
  form.value = emptyForm()
  clearSmtpPassword.value = false
  clearTelegramToken.value = false
}

function buildPayload() {
  const raw = { ...form.value }
  // 凭证三态：勾选清除 → null；填写新值 → 新值；留空 → 不发送（保持原值）
  const smtpPassword = form.value.emailSmtpPassword?.trim()
  if (clearSmtpPassword.value) {
    raw.emailSmtpPassword = null
  } else if (smtpPassword) {
    raw.emailSmtpPassword = smtpPassword
  } else {
    delete raw.emailSmtpPassword
  }
  const telegramToken = form.value.telegramBotToken?.trim()
  if (clearTelegramToken.value) {
    raw.telegramBotToken = null
  } else if (telegramToken) {
    raw.telegramBotToken = telegramToken
  } else {
    delete raw.telegramBotToken
  }
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
    message.error(error instanceof Error ? error.message : '加载通知配置失败')
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
      body: JSON.stringify(buildPayload()),
    })
    cancelEdit()
    await loadConfigs()
  } catch (error) {
    message.error(error instanceof Error ? error.message : '保存通知配置失败')
  } finally {
    saving.value = false
  }
}

async function testConfig(config: NotificationConfig) {
  testingId.value = config.id
  try {
    const response = await request(apiUrl(`/notifications/${config.id}/test`), {
      method: 'POST',
    })
    const result = await responseData<{ success: boolean; message?: string }>(response)
    const resultMessage = result.message || '测试完成'
    testResults.value = {
      ...testResults.value,
      [config.id]: {
        success: result.success,
        message: resultMessage,
        testedAt: new Date().toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' }),
      },
    }
    if (!result.success) message.error(resultMessage)
  } catch (error) {
    const resultMessage = error instanceof Error ? error.message : '测试通知失败'
    testResults.value = {
      ...testResults.value,
      [config.id]: {
        success: false,
        message: resultMessage,
        testedAt: new Date().toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' }),
      },
    }
    message.error(resultMessage)
  } finally {
    testingId.value = ''
  }
}

async function deleteConfig(id: string) {
  try {
    await request(apiUrl(`/notifications/${id}`), {
      method: 'DELETE',
    })
    message.success('已删除通知配置')
    await loadConfigs()
  } catch (error) {
    message.error(error instanceof Error ? error.message : '删除通知配置失败')
  }
}

onMounted(loadConfigs)
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

.notification-form {
  margin-bottom: 16px;
}

.form-title {
  margin: 0;
  font-size: 15px;
  font-weight: 700;
}

.clear-option {
  margin-bottom: 12px;
}

.preview-grid {
  margin-top: 4px;
}

.form-error {
  margin-top: 12px;
}

.form-actions {
  margin-top: 14px;
}

.state-hint {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  padding: 40px 0;
}

.notification-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.config-main p {
  margin: 4px 0;
  font-size: 13px;
}

.test-result {
  margin-top: 8px;
}

.note {
  color: v-bind('themeVars.textColor2');
}

.muted {
  color: v-bind('themeVars.textColor3');
}
</style>
