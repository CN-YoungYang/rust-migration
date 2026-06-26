<template>
  <section class="notification-panel">
    <div class="panel-header">
      <h2>通知设置</h2>
      <button class="primary" @click="startCreate">新建通知</button>
    </div>

    <form v-if="editing" class="notification-form" @submit.prevent="saveConfig">
      <div class="form-row">
        <label>
          通知类型
          <select v-model="form.notifyType" :disabled="Boolean(form.id)">
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
          <input v-model.number="form.failureThreshold" type="number" min="1" max="100" />
        </label>
      </div>

      <div class="form-row">
        <label class="switch-row">
          余额过低通知
          <input v-model="form.onBalanceLow" type="checkbox" />
        </label>
        <label>
          余额阈值（美元）
          <input v-model.number="form.balanceThreshold" type="number" min="0" step="0.01" />
        </label>
      </div>

      <template v-if="form.notifyType === 'webhook'">
        <label>
          Webhook URL
          <input v-model.trim="form.webhookUrl" type="url" />
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
            <input v-model.trim="form.webhookHeaders" placeholder='{"X-Token":"..."}' />
          </label>
        </div>
      </template>

      <template v-if="form.notifyType === 'telegram'">
        <label>
          Bot Token
          <input v-model.trim="form.telegramBotToken" type="password" autocomplete="new-password" />
        </label>
        <label>
          Chat ID
          <input v-model.trim="form.telegramChatId" />
        </label>
      </template>

      <template v-if="form.notifyType === 'email'">
        <div class="form-row">
          <label>
            SMTP 主机
            <input v-model.trim="form.emailSmtpHost" />
          </label>
          <label>
            SMTP 端口
            <input v-model.number="form.emailSmtpPort" type="number" min="1" max="65535" />
          </label>
        </div>
        <label>
          SMTP 用户名
          <input v-model.trim="form.emailSmtpUser" />
        </label>
        <label>
          SMTP 密码
          <input v-model.trim="form.emailSmtpPassword" type="password" autocomplete="new-password" />
        </label>
        <div class="form-row">
          <label>
            发件人
            <input v-model.trim="form.emailFrom" type="email" />
          </label>
          <label>
            收件人
            <input v-model.trim="form.emailTo" />
          </label>
        </div>
      </template>

      <label>
        备注
        <input v-model.trim="form.note" />
      </label>

      <div class="form-actions">
        <button type="submit" class="primary" :disabled="saving">{{ saving ? '保存中...' : '保存' }}</button>
        <button type="button" @click="cancelEdit">取消</button>
      </div>
    </form>

    <div v-if="loading" class="empty">加载中...</div>
    <div v-else-if="configs.length === 0" class="empty">暂无通知配置</div>

    <div v-else class="notification-list">
      <article v-for="config in configs" :key="config.id" class="notification-card">
        <div class="config-main">
          <div class="title-row">
            <strong>{{ typeLabel(config.notifyType) }}</strong>
            <span class="badge" :class="{ disabled: !config.enabled }">{{ config.enabled ? '启用' : '停用' }}</span>
          </div>
          <p class="muted">
            失败阈值：{{ config.failureThreshold }} 次
            <span v-if="config.onBalanceLow"> · 余额阈值：${{ config.balanceThreshold ?? 0 }}</span>
          </p>
          <p v-if="config.note" class="note">{{ config.note }}</p>
        </div>
        <div class="actions">
          <button @click="testConfig(config)" :disabled="testingId === config.id">
            {{ testingId === config.id ? '测试中...' : '测试' }}
          </button>
          <button @click="startEdit(config)">编辑</button>
          <button class="danger" @click="deleteConfig(config.id)">删除</button>
        </div>
      </article>
    </div>
  </section>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { apiUrl, authHeaders, request, responseData } from '../utils/api'
import { showToast } from '../utils/toast'

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
  delete raw.id
  return raw
}

async function loadConfigs() {
  loading.value = true
  try {
    const response = await request(apiUrl('/notifications'), { headers: authHeaders() })
    configs.value = await responseData<NotificationConfig[]>(response)
  } catch (error) {
    showToast(error instanceof Error ? error.message : '加载通知配置失败', 'error')
  } finally {
    loading.value = false
  }
}

async function saveConfig() {
  saving.value = true
  try {
    const id = form.value.id
    await request(apiUrl(id ? `/notifications/${id}` : '/notifications'), {
      method: id ? 'PUT' : 'POST',
      headers: { ...authHeaders(), 'Content-Type': 'application/json' },
      body: JSON.stringify(buildPayload())
    })
    showToast('通知配置已保存', 'success')
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
      method: 'POST',
      headers: authHeaders()
    })
    const result = await responseData<{ success: boolean; message?: string }>(response)
    showToast(result.message || '测试完成', result.success ? 'success' : 'error')
  } catch (error) {
    showToast(error instanceof Error ? error.message : '测试通知失败', 'error')
  } finally {
    testingId.value = ''
  }
}

async function deleteConfig(id: string) {
  if (!confirm('确定要删除此通知配置吗？')) return
  try {
    await request(apiUrl(`/notifications/${id}`), {
      method: 'DELETE',
      headers: authHeaders()
    })
    showToast('通知配置已删除', 'success')
    await loadConfigs()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '删除通知配置失败', 'error')
  }
}

onMounted(loadConfigs)
</script>

<style scoped>
.notification-panel { max-width: 1000px; margin: 0 auto; padding: 2rem; }
.panel-header { display: flex; justify-content: space-between; align-items: center; gap: 1rem; margin-bottom: 1.5rem; }
h2 { color: #fff; }
.notification-form,
.notification-card {
  background: #1a1a1a;
  border: 1px solid #333;
  border-radius: 8px;
  padding: 1.25rem;
}
.notification-form { display: grid; gap: 1rem; margin-bottom: 1.5rem; }
.form-row { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
label { color: #d1d5db; display: grid; gap: 0.4rem; }
.switch-row { align-content: center; grid-template-columns: auto 1fr; align-items: center; }
input,
select {
  color: #fff;
  background: #0b1220;
  border: 1px solid #374151;
  border-radius: 4px;
  padding: 0.55rem;
}
.form-actions,
.actions { display: flex; gap: 0.75rem; align-items: center; }
.notification-list { display: grid; gap: 1rem; }
.notification-card { display: flex; justify-content: space-between; gap: 1rem; }
.title-row { display: flex; align-items: center; gap: 0.5rem; margin-bottom: 0.4rem; }
.badge { background: #10b981; border-radius: 999px; padding: 0.15rem 0.5rem; font-size: 0.75rem; }
.badge.disabled { background: #6b7280; }
.muted { color: #9ca3af; margin: 0.25rem 0; }
.note { color: #fbbf24; margin: 0.25rem 0; }
button {
  color: #fff;
  background: #374151;
  border: 0;
  border-radius: 4px;
  padding: 0.5rem 0.85rem;
}
button:disabled { opacity: 0.6; cursor: not-allowed; }
button.primary,
.primary { background: #0070f3; }
button.danger { background: #dc2626; }
.empty { color: #9ca3af; text-align: center; padding: 2rem; }

@media (max-width: 768px) {
  .notification-panel { padding: 1rem; }
  .form-row,
  .notification-card { grid-template-columns: 1fr; display: grid; }
  .actions { flex-wrap: wrap; }
}
</style>
