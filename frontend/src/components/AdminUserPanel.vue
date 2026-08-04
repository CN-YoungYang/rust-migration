<template>
  <div class="admin-user-panel">
    <div class="panel-header">
      <div>
        <h2>用户管理</h2>
        <p class="panel-subtitle">共 {{ users.length }} 个用户</p>
      </div>
    </div>

    <form class="create-form" aria-labelledby="create-user-title" @submit.prevent="createUser">
      <h3 id="create-user-title">创建新用户</h3>
      <div class="form-group">
        <label for="new-user-username">用户名</label>
        <input id="new-user-username" v-model="newUser.username" type="text" autocomplete="username" required :aria-invalid="createSubmitted && createUsernameInvalid" aria-describedby="create-user-error" />
      </div>
      <div class="form-group">
        <label for="new-user-password">密码（至少 8 位）</label>
        <input id="new-user-password" v-model="newUser.password" type="password" autocomplete="new-password" required minlength="8" :aria-invalid="createSubmitted && createPasswordInvalid" aria-describedby="create-user-error" />
      </div>
      <div class="form-group">
        <label for="new-user-role">角色</label>
        <select id="new-user-role" v-model="newUser.role" required>
          <option value="USER">普通用户</option>
          <option v-if="isSuperAdmin()" value="ADMIN">管理员</option>
        </select>
      </div>
      <div class="form-group">
        <label class="label-check">
          <input v-model="newUser.enabled" type="checkbox" />
          启用
        </label>
      </div>
      <div class="form-group">
        <label for="new-user-note">备注</label>
        <input id="new-user-note" v-model="newUser.note" type="text" placeholder="可选，方便管理员标识用户" />
      </div>
      <button type="submit" class="btn-primary" :disabled="creating" :data-state="creating ? 'loading' : undefined">
        {{ creating ? '创建中...' : '创建用户' }}
      </button>
      <p id="create-user-error" class="field-error-slot" :class="{ 'is-empty': !createErrorMessage }" :role="createErrorMessage ? 'alert' : undefined">{{ createErrorMessage || '\u00a0' }}</p>
    </form>

    <div class="user-list" :aria-busy="loading">
      <h3>用户列表</h3>
      <p v-if="loading" class="empty" role="status" aria-live="polite">加载中...</p>
      <p v-if="!loading && users.length === 0" class="empty" role="status">暂无用户</p>
      <div v-for="user in users" :key="user.id" class="user-card" :class="{ disabled: !user.enabled }">
        <div class="user-main">
          <div class="user-info">
            <strong>{{ user.username }}</strong>
            <span class="badge">{{ roleText(user.role) }}</span>
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
          <button @click="editUser(user)" :disabled="!canManage(user)">编辑</button>
          <button
            @click="deleteUser(user.id)"
            class="btn-delete"
            :disabled="!canManage(user) || user.id === currentUser?.id"
          >
            删除
          </button>
        </div>
      </div>
    </div>

    <Teleport to="body">
      <div v-if="editingUser" class="modal" role="presentation" @click.self="closeEditModal" @keydown.escape="closeEditModal">
        <div v-focus-trap class="modal-content" role="dialog" aria-modal="true" aria-labelledby="edit-user-title" tabindex="-1">
        <h3 id="edit-user-title">编辑用户</h3>
        <form @submit.prevent="updateUser">
          <div class="form-group">
            <label for="edit-user-username">用户名</label>
            <input id="edit-user-username" v-model="editingUser.username" type="text" autocomplete="username" disabled />
          </div>
          <div class="form-group">
            <label for="edit-user-password">新密码（留空则不修改，至少 8 位）</label>
            <input id="edit-user-password" v-model="editingUser.password" type="password" autocomplete="new-password" minlength="8" :aria-invalid="editSubmitted && editPasswordInvalid" aria-describedby="edit-user-error" />
          </div>
          <div v-if="editingUser.id === currentUser?.id" class="form-group">
            <label>角色</label>
            <input :value="roleText(editingUser.role)" disabled />
          </div>
          <div v-else class="form-group">
            <label for="edit-user-role">角色</label>
            <select id="edit-user-role" v-model="editingUser.role" required :aria-invalid="editSubmitted && editRoleInvalid" aria-describedby="edit-user-error">
              <option value="USER">普通用户</option>
              <option v-if="isSuperAdmin()" value="ADMIN">管理员</option>
            </select>
          </div>
          <div class="form-group">
            <label class="label-check">
              <input v-model="editingUser.enabled" type="checkbox" :disabled="editingUser.id === currentUser?.id" />
              启用
            </label>
            <p v-if="!editingUser.enabled" class="disabled-hint">
              禁用后，该用户的账户不会参与自动签到。
            </p>
            <p v-if="editingUser.id === currentUser?.id" class="disabled-hint">
              不能禁用当前登录账号，防止唯一管理员被禁用后无法恢复。
            </p>
          </div>
          <div class="form-group">
            <label for="edit-user-note">备注</label>
            <input id="edit-user-note" v-model="editingUser.note" type="text" placeholder="可选，方便管理员标识用户" />
          </div>
          <div class="modal-actions">
            <button type="submit" class="btn-primary" :disabled="saving" :data-state="saving ? 'loading' : undefined">
              {{ saving ? '保存中...' : '保存' }}
            </button>
            <button type="button" @click="closeEditModal" :disabled="saving">取消</button>
          </div>
          <p id="edit-user-error" class="field-error-slot" :class="{ 'is-empty': !editErrorMessage }" :role="editErrorMessage ? 'alert' : undefined">{{ editErrorMessage || '\u00a0' }}</p>
        </form>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted } from 'vue'
import { apiUrl, request, responseData } from '../utils/api'
import { confirmAction, showToast } from '../utils/toast'
import { vFocusTrap } from '../utils/dialogFocus'
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
const createSubmitted = ref(false)
const editSubmitted = ref(false)
const newUser = ref({
  username: '',
  password: '',
  role: 'USER',
  enabled: true,
  note: ''
})
const editingUser = ref<User | null>(null)

const createErrorMessage = computed(() => {
  if (!createSubmitted.value) return ''
  if (!newUser.value.username.trim()) return '请输入用户名。'
  if (newUser.value.password.length < 8) return '密码至少需要 8 位。'
  return ''
})
const createUsernameInvalid = computed(() => !newUser.value.username.trim())
const createPasswordInvalid = computed(() => newUser.value.password.length < 8)

const editErrorMessage = computed(() => {
  if (!editSubmitted.value || !editingUser.value) return ''
  if (!editingUser.value.role) return '请选择角色。'
  if (editingUser.value.password && editingUser.value.password.length < 8) return '新密码至少需要 8 位。'
  return ''
})
const editRoleInvalid = computed(() => !editingUser.value?.role)
const editPasswordInvalid = computed(() => Boolean(editingUser.value?.password) && (editingUser.value?.password?.length || 0) < 8)

const isSuperAdmin = () => props.currentUser?.role === 'SUPER_ADMIN'

const canManage = (user: User) => {
  // M14：允许管理员/超管编辑自己的账号（改密码/启停/备注）；角色不可自改
  if (props.currentUser?.role === 'SUPER_ADMIN') {
    return user.role !== 'SUPER_ADMIN' || user.id === props.currentUser.id
  }
  if (props.currentUser?.role === 'ADMIN') {
    return user.role === 'USER' || user.id === props.currentUser.id
  }
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
    const res = await request(apiUrl('/admin/users'))
    users.value = await responseData<User[]>(res)
  } catch (error) {
    showToast(error instanceof Error ? error.message : '加载用户失败', 'error')
  } finally {
    loading.value = false
  }
}

const createUser = async () => {
  if (creating.value) return
  createSubmitted.value = true
  if (createErrorMessage.value) return
  creating.value = true
  try {
    await request(apiUrl('/admin/users'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(newUser.value)
    })
    newUser.value = { username: '', password: '', role: 'USER', enabled: true, note: '' }
    createSubmitted.value = false
    await fetchUsers()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '创建用户失败', 'error')
  } finally {
    creating.value = false
  }
}

const editUser = (user: User) => {
  if (!canManage(user)) return
  editSubmitted.value = false
  editingUser.value = { ...user, password: ''}
}

const closeEditModal = () => {
  if (saving.value) return
  editSubmitted.value = false
  editingUser.value = null
}

const updateUser = async () => {
  if (!editingUser.value) return
  if (saving.value) return
  editSubmitted.value = true
  if (editErrorMessage.value) return
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
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    })
    editingUser.value = null
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
    await request(apiUrl(`/admin/users/${id}`), { method: 'DELETE' })
    await fetchUsers()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '删除用户失败', 'error')
  }
}

onMounted(fetchUsers)
</script>

<style scoped src="./AdminUserPanel.css"></style>
