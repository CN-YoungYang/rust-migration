<template>
  <div class="checkin-runs-panel">
    <div class="header">
      <h2>签到记录</h2>
      <div class="header-actions">
        <select v-model="selectedAccountId">
          <option value="">选择账户</option>
          <option v-for="account in accounts" :key="account.id" :value="account.id">
            {{ account.name }} ({{ account.siteType }})
          </option>
        </select>
        <button @click="executeCheckin" class="btn-execute" :disabled="!selectedAccountId">执行签到</button>
        <input v-model.number="keepLatest" type="number" min="1" max="10000" class="keep-input" title="保留最新记录数" />
        <button @click="cleanupRuns" class="btn-cleanup">清理记录</button>
      </div>
    </div>


    <div class="runs-list">
      <div v-for="run in runs" :key="run.id" class="run-card" :class="run.status.toLowerCase()">
        <div class="run-info">
          <strong>{{ accountName(run.accountId) }}</strong>
          <span class="badge" :class="run.status.toLowerCase()">{{ statusText(run.status) }}</span>
          <div class="run-meta">
            <span>触发方式: {{ run.triggeredBy }}</span>
            <span>时间: {{ formatTime(run.createdAt) }}</span>
            <span v-if="run.durationMs">耗时: {{ run.durationMs }}ms</span>
            <span v-if="run.message">消息: {{ run.message }}</span>
          </div>
        </div>
      </div>
      <div v-if="runs.length === 0" class="empty">暂无签到记录</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { apiUrl, authHeaders, request } from '../utils/api'
import { confirmAction, showToast } from '../utils/toast'

interface Account {
  id: string
  name: string
  siteType: string
}

interface CheckinRun {
  id: string
  accountId: string
  status: string
  message?: string | null
  durationMs?: number | null
  triggeredBy: string
  rawResponse?: string | null
  createdAt: string
}

const accounts = ref<Account[]>([])
const runs = ref<CheckinRun[]>([])
const selectedAccountId = ref('')
const keepLatest = ref(100)

const fetchAccounts = async () => {
  const response = await request(apiUrl('/accounts'), { headers: authHeaders() })
  accounts.value = await response.json()
  if (!selectedAccountId.value && accounts.value.length > 0) {
    selectedAccountId.value = accounts.value[0].id
  }
}

const fetchRuns = async () => {
  const response = await request(apiUrl('/checkin-runs'), { headers: authHeaders() })
  runs.value = await response.json()
}

const executeCheckin = async () => {
  if (!selectedAccountId.value) return
  try {
    await request(apiUrl('/checkin-runs'), {
      method: 'POST',
      headers: { ...authHeaders(), 'Content-Type': 'application/json' },
      body: JSON.stringify({ accountId: selectedAccountId.value })
    })
    showToast('签到已执行', 'success')
    await fetchRuns()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '执行签到失败', 'error')
  }
}

const cleanupRuns = async () => {
  if (!(await confirmAction(`确定清理记录并保留最新 ${keepLatest.value} 条吗？`))) return
  try {
    await request(apiUrl('/checkin-runs/cleanup'), {
      method: 'POST',
      headers: { ...authHeaders(), 'Content-Type': 'application/json' },
      body: JSON.stringify({ keepLatest: keepLatest.value })
    })
    showToast('记录已清理', 'success')
    await fetchRuns()
  } catch (error) {
    showToast(error instanceof Error ? error.message : '清理记录失败', 'error')
  }
}

const accountName = (accountId: string) => {
  return accounts.value.find((account) => account.id === accountId)?.name || accountId
}

const statusText = (status: string) => {
  const normalized = status.toLowerCase()
  const map: Record<string, string> = {
    success: '成功',
    failed: '失败',
    already_checked: '今日已签',
    pending: '进行中'
  }
  return map[normalized] || status
}

const formatTime = (time: string) => new Date(time).toLocaleString('zh-CN')

onMounted(async () => {
  try {
    await Promise.all([fetchAccounts(), fetchRuns()])
  } catch (error) {
    showToast(error instanceof Error ? error.message : '加载失败', 'error')
  }
})
</script>

<style scoped>
.checkin-runs-panel { max-width: 1200px; margin: 0 auto; padding: 2rem; }
.header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 2rem; gap: 1rem; }
.header-actions { display: flex; gap: .75rem; align-items: center; flex-wrap: wrap; }
h2 { color: #fff; }
select, input { background: #111827; color: #fff; border: 1px solid #374151; border-radius: 4px; padding: .5rem; }
.keep-input { width: 90px; }
.runs-list { display: grid; gap: 1rem; }
.run-card { background: #1a1a1a; padding: 1.5rem; border-radius: 8px; border-left: 4px solid #666; }
.run-card.success { border-left-color: #10b981; }
.run-card.failed { border-left-color: #ef4444; }
.run-card.already_checked { border-left-color: #3b82f6; }
.run-card.pending { border-left-color: #f59e0b; }
.run-info strong { color: #fff; font-size: 1.1rem; }
.run-info { display: flex; flex-direction: column; gap: 0.5rem; }
.run-meta { display: flex; flex-direction: column; gap: 0.25rem; color: #888; font-size: 0.9rem; }
.badge { padding: 0.25rem 0.5rem; border-radius: 4px; font-size: 0.75rem; display: inline-block; width: fit-content; background: #666; color: white; }
.badge.success { background: #10b981; }
.badge.failed { background: #ef4444; }
.badge.already_checked { background: #3b82f6; }
.badge.pending { background: #f59e0b; }
button { color: white; border: none; padding: 0.5rem 1rem; border-radius: 4px; cursor: pointer; }
button:disabled { background: #666 !important; cursor: not-allowed; }
.btn-execute { background: #0070f3; }
.btn-cleanup { background: #ef4444; }
.empty { text-align: center; color: #666; padding: 3rem; background: #1a1a1a; border-radius: 8px; }
</style>
