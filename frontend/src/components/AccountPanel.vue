<template>
  <section class="account-panel">
    <div class="panel-header">
      <h2>签到账户管理</h2>
      <button class="primary" @click="openCreate">新增账户</button>
    </div>

    <p v-if="loading" class="empty">加载中...</p>

    <div v-if="!loading" class="account-list">
      <article v-for="account in accounts" :key="account.id" class="account-card">
        <div>
          <div class="title-row">
            <strong>{{ account.name }}</strong>
            <span class="badge">{{ account.siteType }}</span>
            <span v-if="!account.enabled" class="badge disabled">已禁用</span>
          </div>
          <p class="muted">{{ account.baseUrl }}</p>
          <p class="muted">认证：{{ account.authType }} ｜ 余额：{{ formatBalance(account.lastBalance) }}</p>
          <p v-if="account.lastStatus" class="muted">最近状态：{{ account.lastStatus }} {{ account.lastMessage || '' }}</p>
        </div>
        <div class="actions">
          <button @click="refreshBalance(account.id)">刷新余额</button>
          <button @click="openEdit(account)">编辑</button>
          <button class="danger" @click="deleteAccount(account.id)">删除</button>
        </div>
      </article>
      <p v-if="accounts.length === 0" class="empty">暂无账户</p>
    </div>

    <div v-if="showForm" class="modal">
      <form class="modal-content" @submit.prevent="submitForm">
        <h3>{{ editingId ? '编辑账户' : '新增账户' }}</h3>
        <label>名称<input v-model="form.name" required /></label>
        <label>站点类型
          <select v-model="form.siteType">
            <option value="new-api">new-api</option>
            <option value="anyrouter">anyrouter</option>
            <option value="x666">x666</option>
          </select>
        </label>
        <label>站点地址<input v-model="form.baseUrl" required /></label>
        <label>用户ID<input v-model="form.userId" /></label>
        <label>认证方式
          <select v-model="form.authType">
            <option value="access_token">access_token</option>
            <option value="cookie">cookie</option>
          </select>
        </label>
        <label>Access Token<input v-model="form.accessToken" type="password" /></label>
        <label>Cookie<textarea v-model="form.cookie" rows="3"></textarea></label>
        <label>自定义签到URL<input v-model="form.customCheckinUrl" /></label>
        <label class="inline"><input v-model="form.enabled" type="checkbox" /> 启用</label>
        <label class="inline"><input v-model="form.retryEnabled" type="checkbox" /> 允许重试</label>
        <div class="modal-actions">
          <button class="primary" type="submit">保存</button>
          <button type="button" @click="closeForm">取消</button>
        </div>
      </form>
    </div>
  </section>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { apiUrl, authHeaders, request } from '../utils/api'
import { confirmAction, showToast } from '../utils/toast'

type Account = {
  id: string
  name: string
  siteType: string
  baseUrl: string
  userId?: string | null
  authType: string
  enabled: boolean
  retryEnabled?: boolean
  lastBalance?: number | string | null
  lastStatus?: string | null
  lastMessage?: string | null
  customCheckinUrl?: string | null
}

function formatBalance(value: number | string | null | undefined): string {
  if (value === null || value === undefined || value === '') return '-'
  const num = typeof value === 'string' ? parseFloat(value) : value
  if (!isFinite(num)) return '-'
  return "$" + num.toFixed(2)
}

const accounts = ref<Account[]>([])
const loading = ref(false)
const showForm = ref(false)
const editingId = ref('')

const form = reactive({
  name: '',
  siteType: 'new-api',
  baseUrl: '',
  userId: '',
  authType: 'access_token',
  accessToken: '',
  cookie: '',
  customCheckinUrl: '',
  enabled: true,
  retryEnabled: true,
})

function resetForm() {
  Object.assign(form, {
    name: '',
    siteType: 'new-api',
    baseUrl: '',
    userId: '',
    authType: 'access_token',
    accessToken: '',
    cookie: '',
    customCheckinUrl: '',
    enabled: true,
    retryEnabled: true,
  })
}

async function loadAccounts() {
  loading.value = true
  try {
    const response = await request(apiUrl('/accounts'), { headers: authHeaders() })
    accounts.value = await response.json()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '加载账户失败', 'error')
  } finally {
    loading.value = false
  }
}

function openCreate() {
  editingId.value = ''
  resetForm()
  showForm.value = true
}

function openEdit(account: Account) {
  editingId.value = account.id
  Object.assign(form, {
    name: account.name,
    siteType: account.siteType,
    baseUrl: account.baseUrl,
    userId: account.userId || '',
    authType: account.authType,
    accessToken: '',
    cookie: '',
    customCheckinUrl: account.customCheckinUrl || '',
    enabled: account.enabled,
    retryEnabled: account.retryEnabled ?? true,
  })
  showForm.value = true
}

function closeForm() {
  showForm.value = false
  editingId.value = ''
}

async function submitForm() {
  const payload = {
    name: form.name,
    siteType: form.siteType,
    baseUrl: form.baseUrl,
    userId: form.userId || undefined,
    authType: form.authType,
    accessToken: form.accessToken || undefined,
    cookie: form.cookie || undefined,
    customCheckinUrl: form.customCheckinUrl || undefined,
    enabled: form.enabled,
    retryEnabled: form.retryEnabled,
  }

  try {
    await request(editingId.value ? apiUrl(`/accounts/${editingId.value}`) : apiUrl('/accounts'), {
      method: editingId.value ? 'PUT' : 'POST',
      headers: { ...authHeaders(), 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    })
    showToast('保存成功', 'success')
    closeForm()
    await loadAccounts()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '保存失败', 'error')
  }
}

async function deleteAccount(id: string) {
  if (!(await confirmAction('确定要删除此账户吗？'))) return
  try {
    await request(apiUrl(`/accounts/${id}`), { method: 'DELETE', headers: authHeaders() })
    showToast('删除成功', 'success')
    await loadAccounts()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '删除失败', 'error')
  }
}

async function refreshBalance(id: string) {
  try {
    await request(apiUrl(`/accounts/${id}/refresh-balance`), { method: 'POST', headers: authHeaders() })
    showToast('余额已刷新', 'success')
    await loadAccounts()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '刷新余额失败', 'error')
  }
}

onMounted(loadAccounts)
</script>

<style scoped>
.account-panel { max-width: 1200px; margin: 0 auto; padding: 2rem; }
.panel-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.5rem; }
.account-list { display: grid; gap: 1rem; }
.account-card { background: #1a1a1a; border: 1px solid #333; border-radius: 8px; padding: 1rem; display: flex; justify-content: space-between; gap: 1rem; }
.title-row { display: flex; gap: .5rem; align-items: center; margin-bottom: .5rem; }
.badge { background: #2563eb; border-radius: 999px; padding: .15rem .5rem; font-size: .75rem; }
.badge.disabled { background: #6b7280; }
.muted { color: #9ca3af; margin: .25rem 0; }
.actions { display: flex; gap: .5rem; align-items: center; }
button { border: 0; border-radius: 4px; padding: .5rem .75rem; cursor: pointer; background: #374151; color: white; }
button.primary, .primary { background: #2563eb; }
button.danger { background: #dc2626; }
.empty { color: #9ca3af; text-align: center; padding: 2rem; }
.modal { position: fixed; inset: 0; background: rgba(0,0,0,.75); display: flex; align-items: center; justify-content: center; z-index: 20; }
.modal-content { width: min(560px, 92vw); max-height: 90vh; overflow: auto; background: #111827; border: 1px solid #374151; border-radius: 10px; padding: 1.5rem; display: grid; gap: .8rem; }
label { display: grid; gap: .35rem; color: #d1d5db; }
label.inline { display: flex; align-items: center; gap: .5rem; }
input, select, textarea { background: #0b1220; border: 1px solid #374151; border-radius: 4px; color: white; padding: .55rem; }
.modal-actions { display: flex; gap: .75rem; justify-content: flex-end; margin-top: .5rem; }
</style>
