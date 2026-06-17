<template>
  <section class="account-panel">
    <div class="panel-header">
      <h2>签到账户管理</h2>
      <div class="header-actions">
        <select v-if="isAdmin" v-model="filterUserId" class="user-filter">
          <option value="">全部用户</option>
          <option v-if="usersLoading" disabled>加载中...</option>
          <option v-for="u in allUsers" :key="u.id" :value="u.id">{{ u.username }}</option>
        </select>
        <button
          v-if="accounts.length > 0"
          class="primary"
          :disabled="batchLoading"
          @click="batchCheckin(accounts.map((a) => a.id))"
        >
          {{ batchLoading ? '签到中...' : (filterUserId ? '该用户签到' : '全部签到') }}
        </button>
        <button class="primary" :disabled="batchLoading" @click="openCreate">新增账户</button>
      </div>
    </div>

    <p v-if="loading" class="empty">加载中...</p>

    <div v-if="!loading" class="account-list">
      <section v-for="group in groupedAccounts" :key="group.key" class="account-group">
        <div class="group-header">
          <h3>{{ group.label }}<span v-if="group.isSelf" class="self-tag">我</span></h3>
          <span class="muted">{{ group.items.length }} 个账户</span>
          <button
            class="batch-btn"
            :disabled="batchLoading"
            @click="batchCheckin(group.items.map((a) => a.id))"
          >
            该组签到
          </button>
        </div>
        <article v-for="account in group.items" :key="account.id" class="account-card">
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
      </section>
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
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { apiUrl, authHeaders, request } from '../utils/api'
import { confirmAction, showToast } from '../utils/toast'

interface CurrentUser {
  id: string
  username: string
  role: string
}

const props = defineProps<{
  currentUser: CurrentUser | null
  isAdmin: boolean
}>()

const allUsers = ref<{ id: string; username: string }[]>([])
const filterUserId = ref('')
const usersLoading = ref(false)

type Account = {
  id: string
  name: string
  siteType: string
  baseUrl: string
  userId?: string | null
  ownerId?: string | null
  ownerName?: string | null
  authType: string
  enabled: boolean
  retryEnabled?: boolean
  lastBalance?: number | string | null
  lastStatus?: string | null
  lastMessage?: string | null
  customCheckinUrl?: string | null
}

interface AccountGroup {
  key: string
  label: string
  isSelf: boolean
  items: Account[]
}

// One API / New API 系列标准换算：500000 quota = 1 美元
// 与 Next.js 版本 (QUOTA_PER_USD = 500000) 保持一致
const QUOTA_PER_USD = 500000

function formatBalance(value: number | string | null | undefined): string {
  if (value === null || value === undefined || value === '') return '余额未刷新'
  const quota = typeof value === 'string' ? parseFloat(value) : value
  if (!isFinite(quota)) return '余额未刷新'
  const usd = quota / QUOTA_PER_USD
  return `${usd.toFixed(2)}`
}

const accounts = ref<Account[]>([])
const loading = ref(false)
const showForm = ref(false)
const editingId = ref('')
const batchLoading = ref(false)

// 批量手动签到：复用分组，跳过今日已签由后端统一判定。
async function batchCheckin(accountIds: string[]) {
  if (accountIds.length === 0 || batchLoading.value) return
  batchLoading.value = true
  try {
    const response = await request(apiUrl('/checkin-runs/batch'), {
      method: 'POST',
      headers: { ...authHeaders(), 'Content-Type': 'application/json' },
      body: JSON.stringify({ accountIds }),
    })
    const result = await response.json() as {
      total: number
      succeeded: number
      skipped: number
      failed: number
    }
    showToast(
      `批量签到完成：成功 ${result.succeeded}，跳过 ${result.skipped}，失败 ${result.failed}`,
      result.failed > 0 ? 'error' : 'success',
    )
    await loadAccounts()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '批量签到失败', 'error')
  } finally {
    batchLoading.value = false
  }
}

// 管理员能看到所有用户的账户，按归属用户分组以免混淆；
// 当前用户自己的分组排在最前，其余按用户名排序。
const groupedAccounts = computed<AccountGroup[]>(() => {
  const groups = new Map<string, AccountGroup>()
  for (const account of accounts.value) {
    const key = account.ownerId || 'unknown'
    if (!groups.has(key)) {
      const label = account.ownerName || (account.ownerId ? `用户 ${account.ownerId.slice(0, 8)}` : '未知用户')
      groups.set(key, {
        key,
        label,
        isSelf: !!props.currentUser && account.ownerId === props.currentUser.id,
        items: [],
      })
    }
    groups.get(key)!.items.push(account)
  }
  return Array.from(groups.values()).sort((a, b) => {
    if (a.isSelf !== b.isSelf) return a.isSelf ? -1 : 1
    return a.label.localeCompare(b.label, 'zh-Hans')
  })
})

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
    let url = apiUrl('/accounts')
    if (props.isAdmin && filterUserId.value) {
      url += `?userId=${encodeURIComponent(filterUserId.value)}`
    }
    const response = await request(url, { headers: authHeaders() })
    accounts.value = await response.json()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '加载账户失败', 'error')
  } finally {
    loading.value = false
  }
}

async function loadUsers() {
  if (!props.isAdmin) return
  usersLoading.value = true
  try {
    const res = await request(apiUrl('/admin/users?scope=all'), { headers: authHeaders() })
    const data = await res.json()
    allUsers.value = data.users ?? data ?? []
  } catch {
    showToast('加载用户列表失败', 'error')
  } finally {
    usersLoading.value = false
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

onMounted(() => {
  loadAccounts()
  loadUsers()
})

watch(filterUserId, () => loadAccounts())
</script>

<style scoped>
.account-panel { max-width: 1200px; margin: 0 auto; padding: 2rem; }
.user-filter { background: #1a2937; color: #fff; border: 1px solid #374151; border-radius: 4px; padding: .4rem .6rem; font-size: .85rem; }
.panel-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.5rem; flex-wrap: wrap; gap: .75rem; }
.header-actions { display: flex; gap: .5rem; align-items: center; flex-wrap: wrap; }
.account-list { display: grid; gap: 1.5rem; }
.account-group { display: grid; gap: 1rem; }
.group-header { display: flex; align-items: center; gap: .6rem; padding-bottom: .25rem; border-bottom: 1px solid #2a2a2a; }
.group-header h3 { margin: 0; font-size: 1rem; color: #e5e7eb; }
.group-header .muted { font-size: .8rem; flex: 1; }
.group-header .batch-btn { background: #10b981; font-size: .8rem; padding: .3rem .7rem; }
.self-tag { background: #2563eb; border-radius: 999px; padding: .05rem .45rem; margin-left: .4rem; font-size: .7rem; color: #fff; }
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
