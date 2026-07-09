<template>
  <div class="admin-user-panel">
    <h2>用户管理</h2>

    <form @submit.prevent="createUser" class="create-form">
      <h3>创建新用户</h3>
      <div class="form-group">
        <label>用户名</label>
        <input v-model="newUser.username" required />
      </div>
      <div class="form-group">
        <label>密码 (至少8位)</label>
        <input v-model="newUser.password" type="password" required minlength="8" />
      </div>
      <div class="form-group">
        <label>角色</label>
        <select v-model="newUser.role" required>
          <option value="USER">普通用户</option>
          <option v-if="isSuperAdmin()" value="ADMIN">管理员</option>
        </select>
      </div>
      <div class="form-group">
        <label>
          <input v-model="newUser.enabled" type="checkbox" />
          启用
        </label>
      </div>
      <div class="form-group">
        <label>备注</label>
        <input v-model="newUser.note" placeholder="可选，方便管理员标识用户" />
      </div>
      <button type="submit" class="btn-primary" :disabled="creating">
        {{ creating ? '创建中...' : '创建用户' }}
      </button>
    </form>

    <div class="user-list">
      <h3>用户列表</h3>
      <p v-if="loading" class="loading-hint">加载中...</p>
      <div v-for="user in users" :key="user.id" class="user-card">
        <div class="user-main">
          <div class="user-info">
            <strong>{{ user.username }}</strong>
            <span class="badge" :class="user.role.toLowerCase()">{{ roleText(user.role) }}</span>
            <span v-if="!user.enabled" class="badge disabled">已禁用</span>
            <span v-if="user.note" class="user-note" :title="user.note">{{ user.note }}</span>
          </div>
          <div class="user-stats">
            <span><b>{{ user.accountCount ?? 0 }}</b> 账户</span>
            <span><b>{{ user.enabledAccountCount ?? 0 }}</b> 启用</span>
            <span :class="{ danger: (user.failedAccountCount ?? 0) > 0 }">
              <b>{{ user.failedAccountCount ?? 0 }}</b> 失败
            </span>
            <span>最近签到：{{ formatDateTime(user.lastRunAt) }}</span>
          </div>
          <p v-if="!user.enabled" class="disabled-hint">
            该用户已禁用，其账户不会参与自动签到。
          </p>
        </div>
        <div class="user-actions">
          <button @click="editUser(user)" class="btn-edit" :disabled="!canManage(user)">编辑</button>
          <button
            @click="deleteUser(user.id)"
            class="btn-delete"
            :disabled="!canManage(user)"
          >
            删除
          </button>
        </div>
      </div>
    </div>

    <div v-if="editingUser" class="modal" @click.self="editingUser = null" @keydown.escape="editingUser = null">
      <div class="modal-content">
        <h3>编辑用户</h3>
        <form @submit.prevent="updateUser">
          <div class="form-group">
            <label>用户名</label>
            <input v-model="editingUser.username" disabled />
          </div>
          <div class="form-group">
            <label>新密码（留空则不修改，至少8位）</label>
            <input v-model="editingUser.password" type="password" minlength="8" />
          </div>
          <div class="form-group">
            <label>角色</label>
            <select v-model="editingUser.role" required>
              <option value="USER">普通用户</option>
              <option v-if="isSuperAdmin()" value="ADMIN">管理员</option>
            </select>
          </div>
          <div class="form-group">
            <label>
              <input v-model="editingUser.enabled" type="checkbox" />
              启用
            </label>
            <p v-if="!editingUser.enabled" class="disabled-hint">
              禁用后，该用户的账户不会参与自动签到。
            </p>
          </div>
          <div class="form-group">
            <label>备注</label>
            <input v-model="editingUser.note" placeholder="可选，方便管理员标识用户" />
          </div>
          <div class="modal-actions">
            <button type="submit" class="btn-primary" :disabled="saving">
              {{ saving ? '保存中...' : '保存' }}
            </button>
            <button type="button" @click="editingUser = null" class="btn-cancel" :disabled="saving">取消</button>
          </div>
        </form>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { apiUrl, authHeaders, request, responseData } from '../utils/api'
import { confirmAction, showToast } from '../utils/toast'
import type { CurrentUser } from '../types'

interface User {
  id: string
  username: string
  role: string
  enabled: boolean
  note?: string | null
  password?: string
  accountCount?: number
  enabledAccountCount?: number
  failedAccountCount?: number
  lastRunAt?: string | null
}

const props = defineProps<{ currentUser: CurrentUser | null }>()

const users = ref<User[]>([])
const loading = ref(false)
const creating = ref(false)
const saving = ref(false)
const newUser = ref({
  username: '',
  password: '',
  role: 'USER',
  enabled: true,
  note: ''
})
const editingUser = ref<User | null>(null)

const isSuperAdmin = () => props.currentUser?.role === 'SUPER_ADMIN'

const canManage = (user: User) => {
  if (props.currentUser?.role === 'SUPER_ADMIN') return user.role !== 'SUPER_ADMIN'
  if (props.currentUser?.role === 'ADMIN') return user.role === 'USER'
  return false
}

const roleText = (role: string) => {
  const map: Record<string, string> = {
    USER: '普通用户',
    ADMIN: '管理员',
    SUPER_ADMIN: '超级管理员'
  }
  return map[role] || role
}

const formatDateTime = (value: string | null | undefined) => {
  if (!value) return '无记录'
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return '无记录'
  return date.toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit'
  })
}

const fetchUsers = async () => {
  loading.value = true
  try {
    const res = await request(apiUrl('/admin/users'), {
      headers: authHeaders()
    })
    users.value = await responseData<User[]>(res)
  } catch (error) {
    showToast(error instanceof Error ? error.message : '加载用户失败', 'error')
  } finally {
    loading.value = false
  }
}

const createUser = async () => {
  if (creating.value) return
  creating.value = true
  try {
    await request(apiUrl('/admin/users'), {
      method: 'POST',
      headers: { ...authHeaders(), 'Content-Type': 'application/json' },
      body: JSON.stringify(newUser.value)
    })
    newUser.value = { username: '', password: '', role: 'USER', enabled: true, note: '' }
    showToast('用户已创建', 'success')
    await fetchUsers()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '创建用户失败', 'error')
  } finally {
    creating.value = false
  }
}

const editUser = (user: User) => {
  if (!canManage(user)) return
  editingUser.value = { ...user, password: ''}
}

const updateUser = async () => {
  if (!editingUser.value) return
  if (saving.value) return
  saving.value = true
  const payload: Record<string, unknown> = {
    role: editingUser.value.role,
    enabled: editingUser.value.enabled,
    note: editingUser.value.note || null
  }
  if (editingUser.value.password) payload.password = editingUser.value.password

  try {
    await request(apiUrl(`/admin/users/${editingUser.value.id}`), {
      method: 'PUT',
      headers: { ...authHeaders(), 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    })
    editingUser.value = null
    showToast('用户已更新', 'success')
    await fetchUsers()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '更新用户失败', 'error')
  } finally {
    saving.value = false
  }
}

const deleteUser = async (id: string) => {
  const user = users.value.find((item) => item.id === id)
  if (!user || !canManage(user)) return
  if (!(await confirmAction('确定要删除此用户吗？'))) return
  try {
    await request(apiUrl(`/admin/users/${id}`), {
      method: 'DELETE',
      headers: authHeaders()
    })
    showToast('用户已删除', 'success')
    await fetchUsers()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '删除用户失败', 'error')
  }
}

onMounted(fetchUsers)
</script>

<style scoped>
.admin-user-panel {
  max-width: 1200px;
  margin: 0 auto;
  padding: 2rem;
}

h2 {
  color: var(--text-strong);
  margin-bottom: 2rem;
}

h3 {
  color: var(--text-strong);
  margin-bottom: 1rem;
}

.create-form {
  background: var(--bg-card);
  border: 1px solid var(--border);
  padding: 1.5rem;
  border-radius: var(--radius);
  margin-bottom: 2rem;
  box-shadow: var(--shadow-card);
}

.form-group {
  margin-bottom: 1rem;
}

.form-group label {
  display: block;
  color: var(--text);
  margin-bottom: 0.5rem;
}

.form-group input[type="text"],
.form-group input[type="password"],
.form-group select {
  width: 100%;
  padding: 0.6rem;
  background: var(--bg-well);
  border: 1px solid var(--border-input);
  border-radius: 6px;
  color: var(--text-strong);
}

.form-group input[type="checkbox"] {
  margin-right: 0.5rem;
}

.btn-primary {
  background: var(--accent);
  color: white;
  border: none;
  padding: 0.5rem 1.5rem;
  border-radius: 6px;
  cursor: pointer;
}

.btn-primary:hover {
  background: var(--accent-hover);
}

.user-list {
  background: var(--bg-card);
  border: 1px solid var(--border);
  padding: 1.5rem;
  border-radius: var(--radius);
}

.user-card {
  background: var(--bg-app);
  border: 1px solid var(--border);
  padding: 1rem;
  border-radius: var(--radius);
  margin-bottom: 0.75rem;
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 1rem;
  transition: background-color 0.16s ease, border-color 0.16s ease;
}

.user-card:hover {
  background: #151f2f;
  border-color: var(--border-strong);
}

.user-info {
  display: flex;
  gap: 0.5rem;
  align-items: center;
  color: var(--text-strong);
}

.user-main {
  display: grid;
  gap: 0.55rem;
  min-width: 0;
}

.user-stats {
  display: flex;
  flex-wrap: wrap;
  gap: 0.55rem 1rem;
  color: var(--text-muted);
  font-size: 0.86rem;
}

.user-stats b {
  color: #e5e7eb;
}

.user-stats .danger,
.user-stats .danger b {
  color: #f87171;
}

.disabled-hint {
  color: var(--warn);
  font-size: 0.85rem;
  margin: 0;
}

.badge {
  padding: 0.25rem 0.5rem;
  border-radius: var(--radius-pill);
  font-size: 0.75rem;
  font-weight: bold;
}

.badge.user {
  background: var(--border-strong);
  color: #dbeafe;
}

.badge.admin {
  background: rgba(245, 158, 11, 0.18);
  color: #fbbf24;
}

.badge.super_admin {
  background: var(--danger-soft);
  color: #f87171;
}

.badge.disabled {
  background: #475569;
  color: var(--text-faint);
}

.user-actions {
  display: flex;
  gap: 0.5rem;
}

.btn-edit {
  background: var(--success);
  color: white;
  border: none;
  padding: 0.25rem 0.75rem;
  border-radius: 6px;
  cursor: pointer;
}

.btn-delete {
  background: #dc2626;
  color: white;
  border: none;
  padding: 0.25rem 0.75rem;
  border-radius: 6px;
  cursor: pointer;
}

.btn-delete:disabled {
  background: #475569;
  cursor: not-allowed;
}

.btn-primary:disabled {
  opacity: 0.65;
  cursor: not-allowed;
}

.modal {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background: rgba(0, 0, 0, 0.8);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal-content {
  background: var(--bg-card);
  border: 1px solid var(--border-input);
  padding: 2rem;
  border-radius: var(--radius);
  width: 90%;
  max-width: 500px;
}

.modal-actions {
  display: flex;
  gap: 1rem;
  margin-top: 1rem;
}

.btn-cancel {
  background: #475569;
  color: white;
  border: none;
  padding: 0.5rem 1.5rem;
  border-radius: 6px;
  cursor: pointer;
}

.loading-hint {
  color: #9ca3af;
  text-align: center;
  padding: 1.5rem;
}

.user-note {
  color: #9ca3af;
  font-size: 0.85rem;
  max-width: 200px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: default;
}

@media (max-width: 768px) {
  .admin-user-panel { padding: 1rem; }
  .user-card { align-items: flex-start; flex-direction: column; }
  .user-info { flex-wrap: wrap; }
  .user-stats { display: grid; gap: 0.35rem; }
  .user-actions { width: 100%; }
  .user-actions button { flex: 1; }
}
</style>
