<template>
  <section class="panel">
    <div class="panel-header">
      <div>
        <h2 class="panel-title">签到账户管理</h2>
        <n-text depth="3" class="panel-subtitle">
          已启用 {{ listSummary.enabled }} 个，今日执行 {{ listSummary.todayRuns }} 次，失败 {{ listSummary.failed }} 个
        </n-text>
      </div>
      <n-space align="center" :size="8">
        <n-select
          v-if="isAdmin"
          v-model:value="filterUserId"
          :options="userFilterOptions"
          placeholder="全部用户"
          :loading="usersLoading"
          size="small"
          style="width: 140px"
        />
        <n-button size="small" :disabled="loading || actionBusy" @click="exportAccounts">
          <template #icon><n-icon :component="DownloadOutline" /></template>
          导出 CSV
        </n-button>
        <n-button size="small" :disabled="actionBusy" @click="openImportDialog">
          <template #icon><n-icon :component="CloudUploadOutline" /></template>
          导入 CSV
        </n-button>
        <n-button
          v-if="accounts.length > 0"
          size="small"
          type="primary"
          :loading="batchLoading"
          :disabled="actionBusy"
          @click="batchCheckin(accounts.map((a) => a.id))"
        >
          {{ batchLoading ? '签到中…' : (filterUserId ? '该用户签到' : '当前列表签到') }}
        </n-button>
        <n-button size="small" type="primary" :disabled="actionBusy" @click="openCreate">
          <template #icon><n-icon :component="AddOutline" /></template>
          新增账户
        </n-button>
      </n-space>
    </div>

    <n-space class="filter-bar" align="center" :size="8" wrap>
      <n-select v-model:value="filterSiteType" :options="siteTypeOptions" size="small" style="width: 130px" />
      <n-select v-model:value="filterEnabled" :options="enabledOptions" size="small" style="width: 120px" />
      <n-select v-model:value="filterLastStatus" :options="lastStatusOptions" size="small" style="width: 140px" />
      <n-input
        v-model:value="filterKeyword"
        type="text"
        clearable
        size="small"
        placeholder="搜索账户名称、地址或备注"
        style="width: 240px"
      />
      <n-button v-if="hasActiveFilter" size="small" @click="clearFilters">清除筛选</n-button>
      <n-text depth="3" class="filter-count">{{ accounts.length }} 个结果</n-text>
    </n-space>

    <n-space v-if="!loading && accounts.length > 0" align="center" :size="8" class="bulk-toolbar">
      <n-text depth="3">已选 {{ selectedIds.length }} 个</n-text>
      <n-button-group size="small">
        <n-button :disabled="selectedIds.length === 0 || actionBusy" @click="batchCheckin(selectedIds)">签到选中</n-button>
        <n-button :disabled="selectedIds.length === 0 || actionBusy" @click="bulkRefreshBalance">刷新余额</n-button>
        <n-button :disabled="selectedIds.length === 0 || actionBusy" @click="bulkSetEnabled(true)">批量启用</n-button>
        <n-button :disabled="selectedIds.length === 0 || actionBusy" @click="bulkSetEnabled(false)">批量禁用</n-button>
      </n-button-group>
      <n-button v-if="selectedIds.length > 0" size="small" text type="primary" :disabled="actionBusy" @click="clearSelection">
        清空选择
      </n-button>
    </n-space>

    <n-card v-if="bulkProgress" size="small" class="progress-panel" role="status" aria-live="polite">
      <div class="progress-meta">
        <strong>{{ bulkProgress.label }}</strong>
        <span>{{ bulkProgress.completed }} / {{ bulkProgress.total }}</span>
      </div>
      <n-progress type="line" :percentage="progressPercent" :height="8" :show-indicator="false" />
      <p v-if="bulkProgress.current" class="muted">当前：{{ bulkProgress.current }}</p>
    </n-card>

    <n-alert
      v-if="bulkErrors.length > 0"
      type="error"
      :show-icon="true"
      closable
      class="error-panel"
      role="alert"
      aria-live="assertive"
      @close="bulkErrors = []"
    >
      <div class="error-panel-title">失败摘要</div>
      <ul class="error-list">
        <li v-for="err in bulkErrors" :key="err">{{ err }}</li>
      </ul>
    </n-alert>

    <n-card v-if="lastBatchResult" size="small" class="batch-result" role="status" aria-live="polite">
      <template #header>
        <div class="batch-result-header">
          <strong>批量签到结果</strong>
          <span class="muted">
            共 {{ lastBatchResult.total }} 个，成功 {{ lastBatchResult.succeeded }} 个，跳过 {{ lastBatchResult.skipped }} 个，失败 {{ lastBatchResult.failed }} 个
          </span>
        </div>
      </template>
      <template #header-extra>
        <n-button size="tiny" text @click="lastBatchResult = null">关闭</n-button>
      </template>
      <n-list>
        <n-list-item v-for="item in lastBatchResult.items" :key="item.accountId">
          <div class="batch-item">
            <span class="batch-name">{{ item.accountName }}</span>
            <n-tag size="small" :bordered="false" :type="checkinStatusTagType(item.status)">
              {{ checkinStatusText(item.status) }}
            </n-tag>
            <span v-if="item.message" class="batch-message muted" :title="item.message">{{ item.message }}</span>
          </div>
        </n-list-item>
      </n-list>
    </n-card>

    <n-data-table
      :columns="columns"
      :data="accounts"
      :loading="loading"
      :row-key="rowKey"
      :checked-row-keys="checkedRowKeys"
      :scroll-x="1080"
      class="accounts-table"
      @update:checked-row-keys="onCheckedRowKeys"
    >
      <template #empty>暂无账户，可使用右上角「新增账户」开始配置。</template>
    </n-data-table>

    <!-- 账户表单弹窗 -->
    <n-modal
      v-model:show="showForm"
      preset="card"
      :title="editingId ? '编辑账户' : '新增账户'"
      style="width: 520px; max-width: 92vw"
      :mask-closable="!formSubmitting"
    >
      <n-form ref="accountFormRef" :model="form" :rules="accountFormRules" label-placement="top">
        <n-form-item label="名称" path="name">
          <n-input v-model:value="form.name" :disabled="formSubmitting" />
        </n-form-item>
        <n-form-item label="站点类型" path="siteType">
          <n-select
            v-model:value="form.siteType"
            :options="siteTypeFormOptions"
            :disabled="Boolean(editingId) || formSubmitting"
          />
        </n-form-item>
        <n-form-item label="站点地址" path="baseUrl">
          <n-input v-model:value="form.baseUrl" :disabled="formSubmitting" />
        </n-form-item>
        <n-form-item v-if="formFields.userId" label="用户ID" path="userId">
          <n-input v-model:value="form.userId" :disabled="formSubmitting" />
        </n-form-item>
        <n-form-item v-if="formFields.authType" label="认证方式" path="authType">
          <n-select
            v-model:value="form.authType"
            :options="authTypeOptions"
            :disabled="Boolean(editingId) || formSubmitting"
          />
        </n-form-item>
        <n-form-item v-if="formFields.accessToken" label="Access Token" path="accessToken">
          <n-input
            v-model:value="form.accessToken"
            type="password"
            show-password-on="click"
            autocomplete="new-password"
            :disabled="formSubmitting"
          />
        </n-form-item>
        <n-form-item v-if="formFields.cookie" label="Cookie" path="cookie">
          <n-input v-model:value="form.cookie" type="textarea" :rows="3" :disabled="formSubmitting" />
        </n-form-item>
        <n-form-item v-if="formFields.customCheckinUrl" label="自定义签到 URL" path="customCheckinUrl">
          <n-input v-model:value="form.customCheckinUrl" placeholder="/api/user/sign_in" :disabled="formSubmitting" />
          <template #feedback>仅支持相对路径，或与站点地址协议、主机和端口完全一致的 URL。</template>
        </n-form-item>
        <n-space :size="24">
          <n-checkbox v-model:checked="form.enabled" :disabled="formSubmitting">启用</n-checkbox>
          <n-checkbox v-model:checked="form.retryEnabled" :disabled="formSubmitting">允许重试</n-checkbox>
        </n-space>
        <n-form-item label="备注" path="note">
          <n-input v-model:value="form.note" placeholder="可选，方便识别账户" :disabled="formSubmitting" />
        </n-form-item>
      </n-form>
      <template #footer>
        <n-space justify="end">
          <n-button :disabled="formSubmitting" @click="closeForm">取消</n-button>
          <n-button type="primary" :loading="formSubmitting" @click="submitForm">
            {{ formSubmitting ? '保存中…' : '保存' }}
          </n-button>
        </n-space>
      </template>
    </n-modal>

    <!-- 批量导入弹窗 -->
    <n-modal
      v-model:show="showImportDialog"
      preset="card"
      title="批量导入账户"
      style="width: 560px; max-width: 92vw"
      :mask-closable="!importing"
    >
      <p class="muted">支持 CSV 格式，需包含 header 行</p>
      <n-alert type="info" :show-icon="true" class="import-instructions">
        <div>必填字段：name, siteType, baseUrl, authType</div>
        <div>可选字段：userId, accessToken, cookie, customCheckinUrl, enabled, retryEnabled, note</div>
      </n-alert>
      <n-collapse>
        <n-collapse-item title="查看示例" name="sample">
          <pre class="import-sample">name,siteType,baseUrl,authType,accessToken,cookie,enabled
测试账户,new-api,https://api.example.com,access_token,sk-xxx,,true</pre>
        </n-collapse-item>
      </n-collapse>
      <n-upload
        :show-file-list="false"
        :default-upload="false"
        accept=".csv"
        class="import-upload"
        @change="handleFileSelect"
      >
        <n-button size="small">{{ selectedFile ? selectedFile.name : '选择 CSV 文件' }}</n-button>
      </n-upload>
      <div v-if="importResult" class="import-result">
        <n-alert v-if="importResult.success > 0" type="success" :show-icon="true">
          成功导入 {{ importResult.success }} 个账户
        </n-alert>
        <n-alert v-if="importResult.failed > 0" type="error" :show-icon="true" class="import-error">
          失败 {{ importResult.failed }} 个
        </n-alert>
        <n-collapse v-if="importResult.errors.length > 0">
          <n-collapse-item title="查看错误详情" name="errors">
            <ul class="error-list">
              <li v-for="(err, idx) in importResult.errors" :key="idx">{{ err }}</li>
            </ul>
          </n-collapse-item>
        </n-collapse>
      </div>
      <template #footer>
        <n-space justify="end">
          <n-button :disabled="importing" @click="closeImportDialog">关闭</n-button>
          <n-button type="primary" :loading="importing" :disabled="!selectedFile" @click="executeImport">
            {{ importing ? '导入中…' : '开始导入' }}
          </n-button>
        </n-space>
      </template>
    </n-modal>
  </section>
</template>

<script setup lang="ts">
import { computed, h, onMounted, onUnmounted, reactive, ref, watch } from 'vue'
import {
  NAlert,
  NButton,
  NButtonGroup,
  NCard,
  NCheckbox,
  NCollapse,
  NCollapseItem,
  NDataTable,
  NForm,
  NFormItem,
  NIcon,
  NInput,
  NList,
  NListItem,
  NModal,
  NPopconfirm,
  NProgress,
  NSelect,
  NSpace,
  NTag,
  NText,
  NUpload,
  useMessage,
  useThemeVars,
  type DataTableColumns,
  type FormInst,
  type FormRules,
  type UploadFileInfo,
} from 'naive-ui'
import { AddOutline, CloudUploadOutline, DownloadOutline } from '@vicons/ionicons5'
import { apiUrl, request, responseData } from '../utils/api'
import { accountFormFields } from '../utils/accountForm'
import { formatDateTime } from '../utils/format'
import { checkinStatusText, checkinStatusTagType } from '../utils/checkinStatus'
import type { CurrentUser, Account } from '../types'
import { useUsers } from '../composables/useUsers'

interface BatchResultItem {
  accountId: string
  accountName: string
  status: string
  message?: string | null
}

interface BatchCheckinResult {
  items: BatchResultItem[]
  total: number
  succeeded: number
  skipped: number
  failed: number
}

interface BulkProgress {
  label: string
  completed: number
  total: number
  current?: string
}

const props = defineProps<{
  currentUser: CurrentUser | null
  isAdmin: boolean
}>()

const message = useMessage()
const themeVars = useThemeVars()
const { allUsers, usersLoading, loadUsers } = useUsers(() => props.isAdmin)

const filterUserId = ref('')
const filterSiteType = ref('')
const filterEnabled = ref('')
const filterLastStatus = ref('')
const filterKeyword = ref('')

const QUOTA_PER_USD = 500000

const accounts = ref<Account[]>([])
const loading = ref(false)
const showForm = ref(false)
const editingId = ref('')
const batchLoading = ref(false)
const bulkLoading = ref(false)
const formSubmitting = ref(false)
const busyAccountIds = ref<Set<string>>(new Set())
const checkedRowKeys = ref<string[]>([])
const bulkProgress = ref<BulkProgress | null>(null)
const bulkErrors = ref<string[]>([])
const lastBatchResult = ref<BatchCheckinResult | null>(null)
let accountRequestSeq = 0

const actionBusy = computed(() => (
  batchLoading.value
  || bulkLoading.value
  || formSubmitting.value
  || busyAccountIds.value.size > 0
))

const visibleAccountIds = computed(() => accounts.value.map((account) => account.id))
const selectedIds = computed(() => checkedRowKeys.value.filter((id) => visibleAccountIds.value.includes(id)))

const listSummary = computed(() => {
  let enabled = 0
  let failed = 0
  let todayRuns = 0
  for (const account of accounts.value) {
    if (account.enabled) enabled += 1
    if (account.lastStatus === 'failed') failed += 1
    todayRuns += account.todayRuns ?? 0
  }
  return { enabled, failed, todayRuns }
})

const progressPercent = computed(() => {
  if (!bulkProgress.value || bulkProgress.value.total === 0) return 0
  return Math.min(100, Math.round((bulkProgress.value.completed / bulkProgress.value.total) * 100))
})

const hasActiveFilter = computed(() => {
  return !!(filterUserId.value || filterSiteType.value || filterEnabled.value || filterLastStatus.value || filterKeyword.value)
})

const siteTypeOptions = [
  { label: '全部类型', value: '' },
  { label: 'new-api', value: 'new-api' },
  { label: 'anyrouter', value: 'anyrouter' },
  { label: 'x666', value: 'x666' },
]

const siteTypeFormOptions = [
  { label: 'new-api', value: 'new-api' },
  { label: 'anyrouter', value: 'anyrouter' },
  { label: 'x666', value: 'x666' },
]

const enabledOptions = [
  { label: '全部状态', value: '' },
  { label: '已启用', value: 'true' },
  { label: '已禁用', value: 'false' },
]

const lastStatusOptions = [
  { label: '全部签到状态', value: '' },
  { label: '今日未签到', value: 'not_today' },
  { label: '成功', value: 'success' },
  { label: '失败', value: 'failed' },
  { label: '今日已签', value: 'already_checked' },
  { label: '从未签到', value: 'never' },
]

const authTypeOptions = [
  { label: 'access_token', value: 'access_token' },
  { label: 'cookie', value: 'cookie' },
]

const userFilterOptions = computed(() => {
  const options = [{ label: '全部用户', value: '' }]
  for (const user of allUsers.value) {
    options.push({ label: user.username, value: user.id })
  }
  return options
})

function formatBalance(value: number | string | null | undefined): string {
  if (value === null || value === undefined || value === '') return '未刷新'
  const quota = typeof value === 'string' ? parseFloat(value) : value
  if (!Number.isFinite(quota)) return '未刷新'
  return `$${(quota / QUOTA_PER_USD).toFixed(2)}`
}

function clearFilters() {
  filterUserId.value = ''
  filterSiteType.value = ''
  filterEnabled.value = ''
  filterLastStatus.value = ''
  filterKeyword.value = ''
}

function pruneSelection() {
  const visible = new Set(visibleAccountIds.value)
  checkedRowKeys.value = checkedRowKeys.value.filter((id) => visible.has(id))
}

function clearSelection() {
  checkedRowKeys.value = []
}

function onCheckedRowKeys(keys: Array<string | number>) {
  checkedRowKeys.value = keys.map(String)
}

function rowKey(row: Account): string {
  return row.id
}

function setAccountBusy(id: string, busy: boolean) {
  const next = new Set(busyAccountIds.value)
  if (busy) {
    next.add(id)
  } else {
    next.delete(id)
  }
  busyAccountIds.value = next
}

function isAccountBusy(id: string): boolean {
  return actionBusy.value || busyAccountIds.value.has(id)
}

function isAccountProcessing(id: string): boolean {
  return busyAccountIds.value.has(id)
}

const columns = computed<DataTableColumns<Account>>(() => {
  // 显式读取这些响应式状态，保证操作中表格自动重渲染
  const busy = busyAccountIds.value
  const busyGlobal = actionBusy.value
  void busy
  void busyGlobal
  void accounts.value

  return [
    { type: 'selection' },
    {
      title: '名称',
      key: 'name',
      render: (row) =>
        h('div', { class: 'account-name-cell' }, [
          h('span', { class: 'account-name' }, row.name),
          row.siteType ? h(NTag, { size: 'small', bordered: false }, { default: () => row.siteType }) : null,
          h('span', { class: 'account-base muted', title: row.baseUrl || '' }, row.baseUrl || '无地址'),
        ]),
    },
    {
      title: '状态',
      key: 'lastStatus',
      width: 110,
      render: (row) => {
        if (!row.enabled) {
          return h(NTag, { size: 'small', bordered: false, type: 'default' }, { default: () => '已禁用' })
        }
        return h(
          NTag,
          { size: 'small', bordered: false, type: checkinStatusTagType(row.lastStatus) },
          { default: () => checkinStatusText(row.lastStatus) },
        )
      },
    },
    { title: '余额', key: 'lastBalance', width: 90, render: (row) => formatBalance(row.lastBalance) },
    { title: '今日', key: 'todayRuns', width: 80, render: (row) => `${row.todayRuns ?? 0} 次` },
    { title: '最近签到', key: 'lastRunAt', width: 130, render: (row) => formatDateTime(row.lastRunAt) },
    {
      title: '最近消息',
      key: 'lastMessage',
      ellipsis: { tooltip: true },
      render: (row) => {
        if (row.lastMessage) return row.lastMessage
        return row.note ? `备注：${row.note}` : '—'
      },
    },
    ...(props.isAdmin
      ? [{ title: '归属', key: 'ownerName', width: 110, render: (row: Account) => row.ownerName || '—' }]
      : []),
    {
      title: '操作',
      key: 'actions',
      width: 300,
      render: (row) =>
        h(NSpace, { size: 4 }, {
          default: () => [
            h(
              NButton,
              {
                size: 'tiny',
                secondary: true,
                loading: isAccountProcessing(row.id),
                disabled: isAccountBusy(row.id),
                onClick: () => refreshBalance(row.id),
              },
              { default: () => '刷新余额' },
            ),
            h(
              NButton,
              {
                size: 'tiny',
                tertiary: true,
                disabled: isAccountBusy(row.id),
                onClick: () => toggleAccountEnabled(row),
              },
              { default: () => (row.enabled ? '禁用' : '启用') },
            ),
            h(
              NButton,
              {
                size: 'tiny',
                tertiary: true,
                disabled: busyGlobal || busy.size > 0,
                onClick: () => openEdit(row),
              },
              { default: () => '编辑' },
            ),
            h(
              NPopconfirm,
              {
                onPositiveClick: () => deleteAccount(row.id),
              },
              {
                trigger: () =>
                  h(
                    NButton,
                    {
                      size: 'tiny',
                      tertiary: true,
                      type: 'error',
                      disabled: busyGlobal || busy.size > 0,
                    },
                    { default: () => '删除' },
                  ),
                default: () => '确定要删除此账户吗？',
              },
            ),
          ],
        }),
    },
  ]
})

function emptyForm() {
  return {
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
    note: '',
  }
}

const form = reactive(emptyForm())
const accountFormRef = ref<FormInst | null>(null)
const formFields = computed(() => accountFormFields(form.siteType, form.authType))

const accountFormRules = computed<FormRules>(() => {
  const rules: FormRules = {
    name: { required: true, message: '请输入账户名称。', trigger: ['blur', 'input'] },
    baseUrl: {
      required: true,
      message: '请输入站点地址。',
      trigger: ['blur', 'input'],
      validator: (_rule, value: string) => {
        if (!value) return new Error('请输入站点地址。')
        try {
          const url = new URL(value)
          if (!['http:', 'https:'].includes(url.protocol)) return new Error('站点地址必须使用 HTTP 或 HTTPS。')
        } catch {
          return new Error('请输入有效的站点地址。')
        }
        return true
      },
    },
  }
  if (!editingId.value && formFields.value.accessToken) {
    rules.accessToken = { required: true, message: '请输入 Access Token。', trigger: ['blur', 'input'] }
  }
  if (!editingId.value && formFields.value.cookie) {
    rules.cookie = { required: true, message: '请输入 Cookie。', trigger: ['blur', 'input'] }
  }
  return rules
})

function resetForm() {
  Object.assign(form, emptyForm())
}

async function loadAccounts() {
  const seq = ++accountRequestSeq
  loading.value = true
  try {
    let url = apiUrl('/accounts')
    const params = new URLSearchParams()

    if (props.isAdmin && filterUserId.value) params.append('userId', filterUserId.value)
    if (filterSiteType.value) params.append('siteType', filterSiteType.value)
    if (filterEnabled.value) params.append('enabled', filterEnabled.value)
    if (filterLastStatus.value) params.append('lastStatus', filterLastStatus.value)
    if (filterKeyword.value) params.append('keyword', filterKeyword.value)

    if (params.toString()) url += `?${params.toString()}`

    const response = await request(url)
    const data = await responseData<Account[]>(response)
    if (seq === accountRequestSeq) {
      accounts.value = data
      pruneSelection()
    }
  } catch (error) {
    if (seq === accountRequestSeq) {
      message.error(error instanceof Error ? error.message : '加载账户失败')
    }
  } finally {
    if (seq === accountRequestSeq) {
      loading.value = false
    }
  }
}

async function batchCheckin(accountIds: readonly string[]) {
  const ids = [...new Set(accountIds)]
  if (ids.length === 0 || batchLoading.value) return

  batchLoading.value = true
  bulkErrors.value = []
  lastBatchResult.value = null
  bulkProgress.value = {
    label: '批量签到',
    completed: 0,
    total: ids.length,
    current: '后端正在按设置串行执行',
  }

  try {
    const response = await request(apiUrl('/checkin-runs/batch'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ accountIds: ids }),
    })
    const result = await responseData<BatchCheckinResult>(response)
    lastBatchResult.value = result
    bulkProgress.value = {
      label: '批量签到',
      completed: result.total,
      total: result.total,
      current: '已完成',
    }
    if (result.failed > 0) {
      message.error(`批量签到有 ${result.failed} 个账户失败`)
    }
    await loadAccounts()
  } catch (error) {
    bulkErrors.value = [error instanceof Error ? error.message : '批量签到失败']
    message.error(bulkErrors.value[0])
  } finally {
    batchLoading.value = false
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
    baseUrl: account.baseUrl || '',
    userId: account.userId || '',
    authType: account.authType || 'access_token',
    accessToken: '',
    cookie: '',
    customCheckinUrl: account.customCheckinUrl || '',
    enabled: account.enabled ?? true,
    retryEnabled: account.retryEnabled ?? true,
    note: account.note || '',
  })
  showForm.value = true
}

function closeForm() {
  if (formSubmitting.value) return
  showForm.value = false
  editingId.value = ''
}

async function submitForm() {
  if (formSubmitting.value) return
  try {
    await accountFormRef.value?.validate()
  } catch {
    return
  }
  formSubmitting.value = true
  const optionalString = (value: string) => {
    const trimmed = value.trim()
    if (trimmed) return trimmed
    return editingId.value ? null : undefined
  }
  const payload = {
    name: form.name,
    siteType: form.siteType,
    baseUrl: form.baseUrl,
    userId: formFields.value.userId ? optionalString(form.userId) : undefined,
    authType: formFields.value.authType ? form.authType : 'cookie',
    accessToken: formFields.value.accessToken ? (form.accessToken.trim() || undefined) : undefined,
    cookie: formFields.value.cookie ? (form.cookie.trim() || undefined) : undefined,
    customCheckinUrl: formFields.value.customCheckinUrl
      ? optionalString(form.customCheckinUrl)
      : undefined,
    enabled: form.enabled,
    retryEnabled: form.retryEnabled,
    note: optionalString(form.note),
  }

  try {
    await request(editingId.value ? apiUrl(`/accounts/${editingId.value}`) : apiUrl('/accounts'), {
      method: editingId.value ? 'PUT' : 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    })
    showForm.value = false
    editingId.value = ''
    await loadAccounts()
  } catch (error) {
    message.error(error instanceof Error ? error.message : '保存失败')
  } finally {
    formSubmitting.value = false
  }
}

async function deleteAccount(id: string) {
  try {
    await request(apiUrl(`/accounts/${id}`), { method: 'DELETE' })
    checkedRowKeys.value = checkedRowKeys.value.filter((key) => key !== id)
    await loadAccounts()
  } catch (error) {
    message.error(error instanceof Error ? error.message : '删除失败')
  }
}

async function refreshBalance(id: string) {
  if (isAccountBusy(id)) return
  setAccountBusy(id, true)
  try {
    await request(apiUrl(`/accounts/${id}/refresh-balance`), { method: 'POST' })
    await loadAccounts()
  } catch (error) {
    message.error(error instanceof Error ? error.message : '刷新余额失败')
  } finally {
    setAccountBusy(id, false)
  }
}

async function updateAccountEnabled(id: string, enabled: boolean) {
  await request(apiUrl(`/accounts/${id}`), {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ enabled }),
  })
}

async function toggleAccountEnabled(account: Account) {
  if (isAccountBusy(account.id)) return
  const nextEnabled = !account.enabled

  setAccountBusy(account.id, true)
  try {
    await updateAccountEnabled(account.id, nextEnabled)
    await loadAccounts()
  } catch (error) {
    message.error(error instanceof Error ? error.message : '更新账户状态失败')
  } finally {
    setAccountBusy(account.id, false)
  }
}

async function bulkRefreshBalance() {
  const ids = selectedIds.value.slice()
  if (ids.length === 0 || bulkLoading.value) return

  bulkLoading.value = true
  bulkErrors.value = []
  let succeeded = 0
  let failed = 0

  try {
    for (const [index, id] of ids.entries()) {
      const account = accounts.value.find((item) => item.id === id)
      bulkProgress.value = {
        label: '批量刷新余额',
        completed: index,
        total: ids.length,
        current: account?.name || id,
      }
      setAccountBusy(id, true)
      try {
        await request(apiUrl(`/accounts/${id}/refresh-balance`), { method: 'POST' })
        succeeded += 1
      } catch (error) {
        failed += 1
        const err = error instanceof Error ? error.message : '刷新失败'
        bulkErrors.value.push(`${account?.name || id}：${err}`)
      } finally {
        setAccountBusy(id, false)
      }
    }
    bulkProgress.value = {
      label: '批量刷新余额',
      completed: ids.length,
      total: ids.length,
      current: '已完成',
    }
    if (failed > 0) {
      message.error(`余额刷新有 ${failed} 个账户失败，成功 ${succeeded} 个`)
    }
    await loadAccounts()
  } finally {
    bulkLoading.value = false
  }
}

async function bulkSetEnabled(enabled: boolean) {
  const ids = selectedIds.value.slice()
  if (ids.length === 0 || bulkLoading.value) return
  const verb = enabled ? '启用' : '禁用'

  bulkLoading.value = true
  bulkErrors.value = []
  let succeeded = 0
  let failed = 0

  try {
    for (const [index, id] of ids.entries()) {
      const account = accounts.value.find((item) => item.id === id)
      bulkProgress.value = {
        label: `批量${verb}`,
        completed: index,
        total: ids.length,
        current: account?.name || id,
      }
      setAccountBusy(id, true)
      try {
        await updateAccountEnabled(id, enabled)
        succeeded += 1
      } catch (error) {
        failed += 1
        const err = error instanceof Error ? error.message : `${verb}失败`
        bulkErrors.value.push(`${account?.name || id}：${err}`)
      } finally {
        setAccountBusy(id, false)
      }
    }
    bulkProgress.value = {
      label: `批量${verb}`,
      completed: ids.length,
      total: ids.length,
      current: '已完成',
    }
    if (failed > 0) {
      message.error(`${verb}操作有 ${failed} 个账户失败，成功 ${succeeded} 个`)
    }
    await loadAccounts()
  } finally {
    bulkLoading.value = false
  }
}

const showImportDialog = ref(false)
const selectedFile = ref<File | null>(null)
const importing = ref(false)
const importResult = ref<{
  success: number
  failed: number
  errors: string[]
} | null>(null)

function openImportDialog() {
  showImportDialog.value = true
  importResult.value = null
  selectedFile.value = null
}

function closeImportDialog() {
  if (importing.value) return
  showImportDialog.value = false
  importResult.value = null
  selectedFile.value = null
}

function handleFileSelect(data: { file: UploadFileInfo }) {
  const raw = data.file.file
  if (raw) {
    selectedFile.value = raw
    importResult.value = null
  }
}

async function executeImport() {
  if (!selectedFile.value) return

  importing.value = true
  try {
    const csvContent = await selectedFile.value.text()
    const response = await request(apiUrl('/accounts/import'), {
      method: 'POST',
      headers: { 'Content-Type': 'text/csv' },
      body: csvContent,
    })

    importResult.value = await responseData<{
      success: number
      failed: number
      errors: string[]
    }>(response)

    if (importResult.value.success > 0) await loadAccounts()

    if (importResult.value.failed > 0) {
      message.error(`${importResult.value.failed} 个账户导入失败`)
    }
  } catch (error) {
    message.error(error instanceof Error ? error.message : '导入失败')
  } finally {
    importing.value = false
  }
}

async function exportAccounts() {
  try {
    const response = await request(apiUrl('/accounts/export'))

    const blob = await response.blob()
    const url = window.URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `ai-hub-accounts-${new Date().toISOString().slice(0, 10)}.csv`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    window.URL.revokeObjectURL(url)

    message.success('导出成功')
  } catch (error) {
    message.error(error instanceof Error ? error.message : '导出失败')
  }
}

onMounted(() => {
  loadAccounts()
  loadUsers()
})

watch(filterUserId, () => loadAccounts())
watch([filterSiteType, filterEnabled, filterLastStatus], () => loadAccounts())

let keywordDebounce: ReturnType<typeof setTimeout> | null = null
watch(filterKeyword, () => {
  if (keywordDebounce) clearTimeout(keywordDebounce)
  keywordDebounce = setTimeout(() => loadAccounts(), 300)
})

onUnmounted(() => {
  if (keywordDebounce) clearTimeout(keywordDebounce)
})
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

.filter-bar {
  margin-bottom: 12px;
}

.bulk-toolbar {
  margin-bottom: 12px;
}

.progress-panel {
  margin-bottom: 12px;
}

.progress-meta {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  margin-bottom: 8px;
}

.error-panel {
  margin-bottom: 12px;
}

.error-panel-title {
  font-weight: 600;
  margin-bottom: 4px;
}

.error-list {
  margin: 0;
  padding-left: 20px;
}

.batch-result {
  margin-bottom: 12px;
}

.batch-result-header {
  display: flex;
  align-items: baseline;
  gap: 10px;
}

.batch-item {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.batch-name {
  font-weight: 500;
  flex: none;
}

.batch-message {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.accounts-table {
  margin-top: 4px;
  font-variant-numeric: tabular-nums;
}

.account-name-cell {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.account-name {
  font-weight: 500;
  display: flex;
  align-items: center;
  gap: 6px;
}

.account-base {
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 220px;
}

.import-instructions {
  margin-bottom: 10px;
}

.import-sample {
  margin: 0;
  font-size: 12px;
  overflow-x: auto;
}

.import-upload {
  margin: 12px 0;
}

.import-error {
  margin-top: 8px;
}

.muted {
  color: v-bind('themeVars.textColor3');
}

.error-list li {
  font-size: 13px;
}
</style>
