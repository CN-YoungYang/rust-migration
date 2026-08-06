<template>
  <div class="panel">
    <div class="panel-header">
      <div class="panel-heading">
        <h2 class="panel-title">用户管理</h2>
        <n-text depth="3" class="panel-subtitle">共 {{ users.length }} 个用户</n-text>
      </div>
      <n-button v-if="!showCreateForm" type="primary" :disabled="loading || creating" @click="openCreateForm">
        <template #icon><n-icon :component="AddOutline" /></template>
        新建用户
      </n-button>
      <n-button v-else quaternary @click="closeCreateForm">收起</n-button>
    </div>

    <!-- 创建用户（折叠，默认收起） -->
    <Transition name="create-fold">
      <n-card v-if="showCreateForm" class="create-form" :bordered="true" aria-labelledby="create-user-title">
        <template #header>
          <div class="create-head">
            <n-icon :component="PersonAddOutline" class="create-head-icon" aria-hidden="true" />
            <h3 id="create-user-title" class="form-title">新建用户</h3>
          </div>
        </template>
        <n-form :model="newUser" label-placement="top" :disabled="creating">
          <n-grid :cols="2" :x-gap="16" responsive="screen" item-responsive>
            <n-grid-item>
              <n-form-item label="用户名" :show-feedback="false">
                <n-input
                  v-model:value="newUser.username"
                  autocomplete="username"
                  :status="createSubmitted && createUsernameInvalid ? 'error' : undefined"
                />
              </n-form-item>
            </n-grid-item>
            <n-grid-item>
              <n-form-item label="密码（至少 8 位）" :show-feedback="false">
                <n-input
                  v-model:value="newUser.password"
                  type="password"
                  show-password-on="click"
                  autocomplete="new-password"
                  :status="createSubmitted && createPasswordInvalid ? 'error' : undefined"
                />
              </n-form-item>
            </n-grid-item>
            <n-grid-item>
              <n-form-item label="角色" :show-feedback="false">
                <n-select v-model:value="newUser.role" :options="roleOptions" />
              </n-form-item>
            </n-grid-item>
            <n-grid-item>
              <n-form-item label="启用" :show-feedback="false">
                <n-checkbox v-model:checked="newUser.enabled">创建后立即启用</n-checkbox>
              </n-form-item>
            </n-grid-item>
          </n-grid>
          <n-form-item label="备注" :show-feedback="false">
            <n-input v-model:value="newUser.note" placeholder="可选，方便管理员标识用户" />
          </n-form-item>

          <n-alert v-if="createErrorMessage" type="error" :show-icon="true" class="form-error" role="alert">
            {{ createErrorMessage }}
          </n-alert>

          <n-space :size="8" class="form-actions">
            <n-button type="primary" :loading="creating" :disabled="creating" @click="createUser">
              {{ creating ? '创建中…' : '创建用户' }}
            </n-button>
            <n-button :disabled="creating" @click="closeCreateForm">取消</n-button>
          </n-space>
        </n-form>
      </n-card>
    </Transition>

    <!-- 筛选栏 + 用户列表 -->
    <div class="user-list" :aria-busy="loading">
      <div class="list-toolbar">
        <h3 class="section-title">用户列表</h3>
        <n-space :size="8" align="center" class="list-filters" :wrap="true">
          <n-select
            v-model:value="roleFilter"
            :options="roleFilterOptions"
            size="small"
            class="filter-select"
            aria-label="按角色筛选"
          />
          <n-select
            v-model:value="statusFilter"
            :options="statusFilterOptions"
            size="small"
            class="filter-select"
            aria-label="按状态筛选"
          />
          <n-input
            v-model:value="keyword"
            size="small"
            clearable
            placeholder="搜索用户名 / 备注"
            class="filter-input"
            aria-label="搜索用户名或备注"
          >
            <template #prefix><n-icon :component="SearchOutline" /></template>
          </n-input>
        </n-space>
      </div>

      <n-data-table
        :columns="userColumns"
        :data="filteredUsers"
        :loading="loading"
        :row-key="(row: User) => row.id"
        :scroll-x="860"
        :row-class-name="(row: User) => row.id === currentUser?.id ? 'is-self' : ''"
      >
        <template #empty>
          <n-empty :description="users.length === 0 ? '暂无用户，点击「新建用户」添加。' : '没有符合筛选条件的用户。'" />
        </template>
      </n-data-table>
    </div>

    <!-- 编辑弹窗 -->
    <n-modal
      v-model:show="editModalVisible"
      preset="card"
      title="编辑用户"
      class="edit-modal"
      :mask-closable="!saving"
      :close-on-esc="!saving"
      @close="closeEditModal"
    >
      <n-form v-if="editingUser" :model="editingUser" label-placement="top" :disabled="saving">
        <n-form-item label="用户名" :show-feedback="false">
          <n-input :value="editingUser.username" disabled />
        </n-form-item>
        <n-form-item label="新密码（留空则不修改，至少 8 位）" :show-feedback="false">
          <n-input
            v-model:value="editingUser.password"
            type="password"
            show-password-on="click"
            autocomplete="new-password"
            :status="editSubmitted && editPasswordInvalid ? 'error' : undefined"
          />
        </n-form-item>
        <n-form-item v-if="editingUser.id === currentUser?.id" label="角色" :show-feedback="false">
          <n-input :value="roleText(editingUser.role)" disabled />
        </n-form-item>
        <n-form-item v-else label="角色" :show-feedback="false">
          <n-select
            v-model:value="editingUser.role"
            :options="roleOptions"
            :status="editSubmitted && editRoleInvalid ? 'error' : undefined"
          />
        </n-form-item>
        <n-form-item label="启用" :show-feedback="false">
          <n-checkbox v-model:checked="editingUser.enabled" :disabled="editingUser.id === currentUser?.id">
            启用该用户
          </n-checkbox>
        </n-form-item>
        <p v-if="!editingUser.enabled" class="hint">禁用后，该用户的账户不会参与自动签到。</p>
        <p v-if="editingUser.id === currentUser?.id" class="hint">不能禁用当前登录账号，防止唯一管理员被禁用后无法恢复。</p>
        <n-form-item label="备注" :show-feedback="false">
          <n-input v-model:value="editingUser.note" placeholder="可选，方便管理员标识用户" />
        </n-form-item>

        <n-alert v-if="editErrorMessage" type="error" :show-icon="true" class="form-error" role="alert">
          {{ editErrorMessage }}
        </n-alert>

        <n-space :size="8" class="modal-actions">
          <n-button type="primary" :loading="saving" :disabled="saving" @click="updateUser">
            {{ saving ? '保存中…' : '保存' }}
          </n-button>
          <n-button :disabled="saving" @click="closeEditModal">取消</n-button>
        </n-space>
      </n-form>
    </n-modal>
  </div>
</template>

<script setup lang="ts">
import { computed, h, onMounted, ref } from 'vue'
import {
  NAlert,
  NButton,
  NCard,
  NCheckbox,
  NDataTable,
  NEmpty,
  NForm,
  NFormItem,
  NGrid,
  NGridItem,
  NIcon,
  NInput,
  NModal,
  NPopconfirm,
  NSelect,
  NSpace,
  NTag,
  NText,
  useMessage,
  useThemeVars,
  type DataTableColumns,
} from 'naive-ui'
import {
  AddOutline,
  CreateOutline,
  PersonAddOutline,
  SearchOutline,
  TrashOutline,
} from '@vicons/ionicons5'
import { apiUrl, request, responseData } from '../utils/api'
import { formatDateTime } from '../utils/format'
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

const message = useMessage()
const themeVars = useThemeVars()
const users = ref<User[]>([])
const loading = ref(false)
const creating = ref(false)
const saving = ref(false)
const deletingId = ref('')
const createSubmitted = ref(false)
const editSubmitted = ref(false)
const editModalVisible = ref(false)
const showCreateForm = ref(false)
const newUser = ref({
  username: '',
  password: '',
  role: 'USER',
  enabled: true,
  note: '',
})
const editingUser = ref<User | null>(null)

// 筛选状态（纯前端，不新增 API）
const roleFilter = ref<'ALL' | 'USER' | 'ADMIN' | 'SUPER_ADMIN'>('ALL')
const statusFilter = ref<'ALL' | 'ENABLED' | 'DISABLED'>('ALL')
const keyword = ref('')

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

const roleOptions = computed(() => {
  const options = [{ label: '普通用户', value: 'USER' }]
  if (isSuperAdmin()) options.push({ label: '管理员', value: 'ADMIN' })
  return options
})

// 筛选下拉选项：角色按当前登录者可见范围动态呈现
const roleFilterOptions = computed(() => {
  const options: { label: string; value: 'ALL' | 'USER' | 'ADMIN' | 'SUPER_ADMIN' }[] = [
    { label: '全部角色', value: 'ALL' },
    { label: '普通用户', value: 'USER' },
  ]
  if (isSuperAdmin()) {
    options.push({ label: '管理员', value: 'ADMIN' })
    options.push({ label: '超级管理员', value: 'SUPER_ADMIN' })
  }
  return options
})

const statusFilterOptions = [
  { label: '全部状态', value: 'ALL' as const },
  { label: '启用', value: 'ENABLED' as const },
  { label: '停用', value: 'DISABLED' as const },
]

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
    SUPER_ADMIN: '超级管理员',
  }
  return map[role] || role
}

function roleTagType(role: string): 'default' | 'info' | 'warning' {
  if (role === 'SUPER_ADMIN') return 'warning'
  if (role === 'ADMIN') return 'info'
  return 'default'
}

// 筛选结果：角色 + 状态 + 关键字三路交集
const filteredUsers = computed(() => {
  const kw = keyword.value.trim().toLowerCase()
  return users.value.filter((user) => {
    if (roleFilter.value !== 'ALL' && user.role !== roleFilter.value) return false
    if (statusFilter.value === 'ENABLED' && !user.enabled) return false
    if (statusFilter.value === 'DISABLED' && user.enabled) return false
    if (kw) {
      const hay = `${user.username}\t${user.note ?? ''}`.toLowerCase()
      if (!hay.includes(kw)) return false
    }
    return true
  })
})

const userColumns = computed<DataTableColumns<User>>(() => {
  // 显式依赖操作中的响应式状态，保证表格重渲染
  void deletingId.value
  void users.value

  return [
    {
      title: '用户名',
      key: 'username',
      render: (user) => {
        const isSelf = user.id === props.currentUser?.id
        return h('div', { class: 'user-name-cell' }, [
          h('div', { class: 'user-name-row' }, [
            h('strong', {}, user.username),
            h(NTag, { size: 'small', bordered: false, type: roleTagType(user.role) }, { default: () => roleText(user.role) }),
            isSelf ? h('span', { class: 'self-mark', title: '当前登录账号' }, '当前账号') : null,
            user.note
              ? h('span', { class: 'user-note', title: user.note }, user.note)
              : null,
          ]),
          !user.enabled ? h('span', { class: 'disabled-hint' }, '已禁用，不参与自动签到') : null,
        ])
      },
    },
    {
      title: '状态',
      key: 'enabled',
      width: 84,
      render: (user) =>
        h(
          NTag,
          { size: 'small', bordered: false, type: user.enabled ? 'success' : 'default' },
          { default: () => (user.enabled ? '启用' : '停用') },
        ),
    },
    {
      title: '账户统计',
      key: 'accounts',
      render: (user) => {
        const failed = user.failedAccountCount ?? 0
        return h('div', { class: 'user-stats' }, [
          h('span', {}, [h('b', {}, user.accountCount ?? 0), ' 账户']),
          h('span', { class: 'muted' }, [h('b', {}, user.enabledAccountCount ?? 0), ' 启用']),
          h(
            'span',
            failed > 0 ? { class: 'stat-failed' } : { class: 'muted' },
            [h('b', {}, failed), ' 失败'],
          ),
        ])
      },
    },
    { title: '最近签到', key: 'lastRunAt', render: (user) => formatDateTime(user.lastRunAt) },
    {
      title: '操作',
      key: 'actions',
      width: 130,
      render: (user) => {
        const manageable = canManage(user)
        const isSelf = user.id === props.currentUser?.id
        return h(NSpace, { size: 4 }, {
          default: () => [
            h(
              NButton,
              {
                size: 'tiny',
                tertiary: true,
                disabled: !manageable,
                onClick: () => editUser(user),
              },
              {
                default: () => '编辑',
                icon: () => h(NIcon, { component: CreateOutline }, { default: () => [] }),
              },
            ),
            h(
              NPopconfirm,
              { onPositiveClick: () => deleteUser(user.id) },
              {
                trigger: () =>
                  h(
                    NButton,
                    {
                      size: 'tiny',
                      tertiary: true,
                      type: 'error',
                      disabled: !manageable || isSelf,
                      loading: deletingId.value === user.id,
                    },
                    {
                      default: () => '删除',
                      icon: () => h(NIcon, { component: TrashOutline }, { default: () => [] }),
                    },
                  ),
                default: () => `确定要删除用户「${user.username}」吗？此操作不可撤销。`,
              },
            ),
          ],
        })
      },
    },
  ]
})

const fetchUsers = async () => {
  loading.value = true
  try {
    const res = await request(apiUrl('/admin/users'))
    users.value = await responseData<User[]>(res)
  } catch (error) {
    message.error(error instanceof Error ? error.message : '加载用户失败')
  } finally {
    loading.value = false
  }
}

const openCreateForm = () => {
  createSubmitted.value = false
  showCreateForm.value = true
}

const closeCreateForm = () => {
  if (creating.value) return
  showCreateForm.value = false
  newUser.value = { username: '', password: '', role: 'USER', enabled: true, note: '' }
  createSubmitted.value = false
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
      body: JSON.stringify(newUser.value),
    })
    newUser.value = { username: '', password: '', role: 'USER', enabled: true, note: '' }
    createSubmitted.value = false
    showCreateForm.value = false
    message.success('已创建用户')
    await fetchUsers()
  } catch (error) {
    message.error(error instanceof Error ? error.message : '创建用户失败')
  } finally {
    creating.value = false
  }
}

const editUser = (user: User) => {
  if (!canManage(user)) return
  editSubmitted.value = false
  editingUser.value = { ...user, password: '' }
  editModalVisible.value = true
}

const closeEditModal = () => {
  if (saving.value) return
  editSubmitted.value = false
  editingUser.value = null
  editModalVisible.value = false
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
    note: editingUser.value.note || null,
  }
  if (editingUser.value.password) payload.password = editingUser.value.password

  try {
    await request(apiUrl(`/admin/users/${editingUser.value.id}`), {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    })
    message.success('已保存用户信息')
    editingUser.value = null
    editModalVisible.value = false
    await fetchUsers()
  } catch (error) {
    message.error(error instanceof Error ? error.message : '更新用户失败')
  } finally {
    saving.value = false
  }
}

const deleteUser = async (id: string) => {
  const user = users.value.find((item) => item.id === id)
  if (!user || !canManage(user)) return
  deletingId.value = id
  try {
    await request(apiUrl(`/admin/users/${id}`), { method: 'DELETE' })
    message.success('已删除用户')
    await fetchUsers()
  } catch (error) {
    message.error(error instanceof Error ? error.message : '删除用户失败')
  } finally {
    deletingId.value = ''
  }
}

onMounted(fetchUsers)
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

.create-form {
  margin-bottom: 20px;
}

.create-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.create-head-icon {
  font-size: 18px;
  color: v-bind('themeVars.primaryColor');
}

.form-title,
.section-title {
  margin: 0;
  font-size: 15px;
  font-weight: 700;
}

.section-title {
  margin: 0;
  white-space: nowrap;
}

.form-error {
  margin-bottom: 14px;
}

.form-actions {
  margin-top: 4px;
}

/* 列表工具栏：标题左、筛选右；窄屏换行 */
.list-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
  margin-bottom: 12px;
}

.list-filters {
  flex-wrap: wrap;
}

.filter-select {
  width: 132px;
}

.filter-input {
  width: 200px;
}

.user-list {
  margin-top: 4px;
  font-variant-numeric: tabular-nums;
}

/* 当前登录账号行：整行轻底色 + 左侧主色细条，避免抢夺主信息 */
.user-list :deep(.n-data-table .n-data-table-tr.is-self .n-data-table-td) {
  background: color-mix(in srgb, v-bind('themeVars.primaryColor') 5%, transparent);
}
.user-list :deep(.n-data-table .n-data-table-tr.is-self) {
  box-shadow: inset 2px 0 0 v-bind('themeVars.primaryColor');
}

.user-name-cell {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.user-name-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.self-mark {
  padding: 0 6px;
  font-size: 11px;
  line-height: 16px;
  border-radius: 4px;
  color: v-bind('themeVars.primaryColor');
  background: color-mix(in srgb, v-bind('themeVars.primaryColor') 12%, transparent);
}

.user-note {
  font-size: 12px;
  color: v-bind('themeVars.textColor3');
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 200px;
}

.disabled-hint {
  margin: 0;
  font-size: 12px;
  color: v-bind('themeVars.warningColor');
}

.user-stats {
  display: flex;
  gap: 12px;
  font-size: 13px;
}

.stat-failed {
  color: v-bind('themeVars.errorColor');
}

.hint {
  margin: 0 0 12px;
  font-size: 12px;
  color: v-bind('themeVars.textColor3');
}

.modal-actions {
  margin-top: 8px;
}

.muted {
  color: v-bind('themeVars.textColor3');
}

/* 创建表单展开/收起：仅动 max-height + opacity，且 reduced-motion 下收敛 */
.create-fold-enter-active,
.create-fold-leave-active {
  transition: max-height 0.2s ease, opacity 0.15s ease, margin 0.2s ease;
  overflow: hidden;
}
.create-fold-enter-from,
.create-fold-leave-to {
  max-height: 0;
  opacity: 0;
  margin-bottom: 0;
}
.create-fold-enter-to,
.create-fold-leave-from {
  max-height: 600px;
  opacity: 1;
}
@media (prefers-reduced-motion: reduce) {
  .create-fold-enter-active,
  .create-fold-leave-active {
    transition: none;
  }
}

/* 窄屏：筛选下拉与搜索框全宽，工具栏退为单列 */
@media (max-width: 720px) {
  .list-toolbar {
    flex-direction: column;
    align-items: stretch;
  }
  .list-filters {
    width: 100%;
  }
  .filter-select,
  .filter-input {
    width: 100%;
  }
}
</style>
