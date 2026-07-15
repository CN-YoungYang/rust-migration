<template>
  <section class="statistics-panel">
    <div class="panel-header">
      <div>
        <h2>数据统计</h2>
        <p class="panel-subtitle">{{ selectedUserName }} · {{ rangeLabel }}</p>
      </div>
      <div class="toolbar">
        <select
          v-if="isAdmin"
          v-model="selectedUserId"
          class="user-filter"
          aria-label="按用户查询统计"
          :disabled="usersLoading || loading"
        >
          <option value="">全部用户</option>
          <option v-if="usersLoading" disabled>加载中...</option>
          <option v-for="u in allUsers" :key="u.id" :value="u.id">
            {{ u.username }}{{ u.id === currentUser?.id ? '（我）' : '' }}
          </option>
        </select>
        <div class="date-range" role="group" aria-label="统计日期范围">
          <input v-model="startDate" type="date" class="date-input" aria-label="开始日期" :disabled="loading" />
          <span class="date-separator">至</span>
          <input v-model="endDate" type="date" class="date-input" aria-label="结束日期" :disabled="loading" />
          <button class="primary" @click="loadStatistics" :disabled="loading || dateRangeInvalid">
            {{ loading ? '查询中...' : '查询' }}
          </button>
          <button @click="applyRange(7)" :disabled="loading">7天</button>
          <button @click="applyRange(30)" :disabled="loading">30天</button>
          <button @click="applyRange(90)" :disabled="loading">90天</button>
        </div>
      </div>
    </div>

    <div v-if="dateRangeInvalid" class="validation-box" role="alert">开始日期不能晚于结束日期。</div>

    <p v-if="loading" class="empty" role="status" aria-live="polite">加载中...</p>

    <div v-if="!loading && statistics" class="stats-content">
      <div class="overview-grid">
        <div class="stat-card">
          <div class="stat-mark">账户</div>
          <div class="stat-info">
            <div class="stat-value">{{ statistics.overview.totalAccounts }}</div>
            <div class="stat-label">总账户数</div>
            <div class="stat-sub">启用率：{{ enabledRatio }}%</div>
          </div>
        </div>

        <div class="stat-card" :class="{ warning: statistics.overview.todayFailed > 0 }">
          <div class="stat-mark">今日</div>
          <div class="stat-info">
            <div class="stat-value">{{ statistics.overview.todaySuccess }}</div>
            <div class="stat-label">今日成功</div>
            <div class="stat-sub">失败：{{ statistics.overview.todayFailed }}，失败率：{{ todayFailureRate }}%</div>
          </div>
        </div>

        <div class="stat-card">
          <div class="stat-mark">成功率</div>
          <div class="stat-info">
            <div class="stat-value">{{ statistics.overview.successRate.toFixed(1) }}%</div>
            <div class="stat-label">区间成功率</div>
            <div class="stat-sub">总计：{{ statistics.overview.totalRuns }} 次</div>
          </div>
        </div>

        <div class="stat-card">
          <div class="stat-mark">余额</div>
          <div class="stat-info">
            <div class="stat-value">${{ statistics.overview.totalBalance.toFixed(2) }}</div>
            <div class="stat-label">总余额</div>
            <div class="stat-sub">已启用账户</div>
          </div>
        </div>
      </div>

      <div class="insight-grid">
        <div class="insight-item">
          <span>最近失败</span>
          <strong>{{ statistics.recentFailures.length }} 条</strong>
        </div>
        <div class="insight-item">
          <span>风险站点</span>
          <strong>{{ highestRiskSite }}</strong>
        </div>
        <div class="insight-item">
          <span>今日执行</span>
          <strong>{{ todayTotal }} 次</strong>
        </div>
      </div>

      <div class="chart-section">
        <h3>每日签到趋势</h3>
        <div v-if="statistics.dailyTrend.length > 0" class="chart-container" role="group" aria-label="每日签到趋势图">
          <div class="chart-legend">
            <span class="legend-item"><span class="dot success"></span>成功</span>
            <span class="legend-item"><span class="dot failed"></span>失败</span>
            <span class="legend-item"><span class="dot already"></span>已签到</span>
          </div>
          <div class="bar-chart" role="list" aria-label="每日签到趋势数据">
            <div v-for="day in statistics.dailyTrend" :key="day.date" class="bar-group" role="listitem" tabindex="0" :aria-label="trendAriaLabel(day)">
              <div class="bar-stack">
                <div
                  class="bar success"
                  :style="{ height: getBarHeight(day.success, day.total) + '%' }"
                  :title="`成功：${day.success}`"
                  aria-hidden="true"
                ></div>
                <div
                  class="bar already"
                  :style="{ height: getBarHeight(day.alreadyChecked, day.total) + '%' }"
                  :title="`已签到：${day.alreadyChecked}`"
                  aria-hidden="true"
                ></div>
                <div
                  class="bar failed"
                  :style="{ height: getBarHeight(day.failed, day.total) + '%' }"
                  :title="`失败：${day.failed}`"
                  aria-hidden="true"
                ></div>
              </div>
              <div class="bar-label">{{ formatDate(day.date) }}</div>
              <div class="bar-value">{{ day.total }}</div>
              <div class="bar-rate">{{ day.successRate.toFixed(0) }}%</div>
            </div>
          </div>
        </div>
        <p v-else class="empty" role="status">所选时间范围内无签到记录</p>
      </div>

      <div class="table-section" tabindex="0" aria-label="站点统计表，可横向滚动">
        <h3>站点统计</h3>
        <table v-if="statistics.siteStats.length > 0" class="stats-table">
          <caption class="sr-only">按站点类型汇总的账户数、签到结果、成功率和平均耗时</caption>
          <thead>
            <tr>
              <th scope="col">站点类型</th>
              <th scope="col">账户数</th>
              <th scope="col">总签到</th>
              <th scope="col">成功</th>
              <th scope="col">失败</th>
              <th scope="col">成功率</th>
              <th scope="col">平均耗时</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="site in statistics.siteStats" :key="site.siteType">
              <td><span class="badge">{{ site.siteType }}</span></td>
              <td>{{ site.accountCount }}</td>
              <td>{{ site.totalRuns }}</td>
              <td class="success-text">{{ site.success }}</td>
              <td class="failed-text">{{ site.failed }}</td>
              <td>
                <span class="rate-badge" :class="getRateClass(site.successRate)">
                  {{ site.successRate.toFixed(1) }}%
                </span>
              </td>
              <td>{{ site.avgDuration.toFixed(0) }}ms</td>
            </tr>
          </tbody>
        </table>
        <p v-else class="empty" role="status">暂无站点统计</p>
      </div>

      <div class="failure-section">
        <h3>最近失败</h3>
        <div v-if="statistics.recentFailures.length > 0" class="failure-list">
          <article v-for="failure in statistics.recentFailures" :key="failure.runId" class="failure-item">
            <div class="failure-main">
              <div class="failure-title">
                <strong>{{ failure.accountName }}</strong>
                <span class="badge">{{ failure.siteType }}</span>
                <span v-if="failure.ownerName" class="owner-tag">{{ failure.ownerName }}</span>
              </div>
              <p class="failure-message" :title="failure.message || ''">
                {{ failure.message || '无错误消息' }}
              </p>
              <p class="muted">{{ formatDateTime(failure.createdAt) }}</p>
            </div>
            <button @click="copyFailureSummary(failure)">复制摘要</button>
          </article>
        </div>
        <p v-else class="empty" role="status">暂无失败记录</p>
      </div>

      <div class="info-section">
        <h3>余额信息</h3>
        <p class="info-text">
          当前总余额：<strong>${{ statistics.overview.totalBalance.toFixed(2) }}</strong>
          （{{ (statistics.overview.totalBalance * 500000).toFixed(0) }} quota）
        </p>
        <p class="muted">余额基于最后一次刷新时间，可能不是实时数据。</p>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, watch } from 'vue'
import { apiUrl, authHeaders, request, responseData } from '../utils/api'
import { showToast } from '../utils/toast'
import type { CurrentUser } from '../types'
import { useUsers } from '../composables/useUsers'

interface Statistics {
  overview: {
    totalAccounts: number
    enabledAccounts: number
    todaySuccess: number
    todayFailed: number
    totalRuns: number
    successRate: number
    totalBalance: number
  }
  dailyTrend: Array<{
    date: string
    success: number
    failed: number
    alreadyChecked: number
    total: number
    successRate: number
  }>
  siteStats: Array<{
    siteType: string
    accountCount: number
    totalRuns: number
    success: number
    failed: number
    successRate: number
    avgDuration: number
  }>
  balanceTrend: Array<{
    date: string
    balance: number
  }>
  recentFailures: Array<{
    runId: string
    accountId: string
    accountName: string
    siteType: string
    ownerName?: string | null
    message?: string | null
    createdAt: string
  }>
}

const props = defineProps<{
  currentUser: CurrentUser | null
  isAdmin: boolean
}>()

const { allUsers, usersLoading, loadUsers } = useUsers(() => props.isAdmin)
const loading = ref(false)
const statistics = ref<Statistics | null>(null)
const startDate = ref('')
const endDate = ref('')
const selectedUserId = ref('')
let requestSeq = 0

const selectedUserName = computed(() => {
  if (!props.isAdmin || !selectedUserId.value) return '全部用户'
  return allUsers.value.find((user) => user.id === selectedUserId.value)?.username || '指定用户'
})

const rangeLabel = computed(() => {
  if (!startDate.value || !endDate.value) return '默认时间范围'
  return `${startDate.value} 至 ${endDate.value}`
})

const dateRangeInvalid = computed(() => {
  return Boolean(startDate.value && endDate.value && startDate.value > endDate.value)
})

const enabledRatio = computed(() => {
  if (!statistics.value || statistics.value.overview.totalAccounts === 0) return '0.0'
  return (statistics.value.overview.enabledAccounts / statistics.value.overview.totalAccounts * 100).toFixed(1)
})

const todayTotal = computed(() => {
  if (!statistics.value) return 0
  return statistics.value.overview.todaySuccess + statistics.value.overview.todayFailed
})

const todayFailureRate = computed(() => {
  if (!statistics.value || todayTotal.value === 0) return '0.0'
  return (statistics.value.overview.todayFailed / todayTotal.value * 100).toFixed(1)
})

const highestRiskSite = computed(() => {
  if (!statistics.value || statistics.value.siteStats.length === 0) return '无'
  const failedSites = statistics.value.siteStats
    .filter((site) => site.failed > 0)
    .sort((a, b) => b.failed - a.failed || a.successRate - b.successRate)
  return failedSites[0]?.siteType || '无'
})

// 设置默认时间范围（最近30天）
function setDefaultRange() {
  const today = new Date()
  const thirtyDaysAgo = new Date(today)
  thirtyDaysAgo.setDate(today.getDate() - 29)

  endDate.value = formatDateInput(today)
  startDate.value = formatDateInput(thirtyDaysAgo)
}

function applyRange(days: number) {
  const today = new Date()
  const start = new Date(today)
  start.setDate(today.getDate() - (days - 1))
  endDate.value = formatDateInput(today)
  startDate.value = formatDateInput(start)
  loadStatistics()
}

function formatDateInput(date: Date): string {
  return date.toISOString().split('T')[0]
}

function formatDate(dateStr: string): string {
  const [, month, day] = dateStr.split('-')
  return `${month}/${day}`
}

function getBarHeight(value: number, total: number): number {
  if (total === 0) return 0
  return (value / total) * 100
}

function trendAriaLabel(day: Statistics['dailyTrend'][number]): string {
  return `${formatDate(day.date)}：总计 ${day.total} 次，成功 ${day.success} 次，已签到 ${day.alreadyChecked} 次，失败 ${day.failed} 次，成功率 ${day.successRate.toFixed(0)}%`
}

function getRateClass(rate: number): string {
  if (rate >= 90) return 'rate-excellent'
  if (rate >= 70) return 'rate-good'
  if (rate >= 50) return 'rate-fair'
  return 'rate-poor'
}

async function loadStatistics() {
  if (dateRangeInvalid.value) {
    showToast('开始日期不能晚于结束日期', 'error')
    return
  }
  const seq = ++requestSeq
  loading.value = true
  try {
    const params = new URLSearchParams()
    if (startDate.value) params.append('startDate', startDate.value)
    if (endDate.value) params.append('endDate', endDate.value)
    if (props.isAdmin && selectedUserId.value) params.append('userId', selectedUserId.value)

    const url = apiUrl(`/statistics?${params.toString()}`)
    const response = await request(url, { headers: authHeaders() })
    const data = await responseData<Statistics>(response)
    if (seq === requestSeq) {
      statistics.value = data
    }
  } catch (error) {
    if (seq === requestSeq) {
      showToast(error instanceof Error ? error.message : '加载统计数据失败', 'error')
    }
  } finally {
    if (seq === requestSeq) {
      loading.value = false
    }
  }
}

function formatDateTime(time: string): string {
  const date = new Date(time)
  if (Number.isNaN(date.getTime())) return '无效时间'
  return date.toLocaleString('zh-CN')
}

async function copyFailureSummary(failure: Statistics['recentFailures'][number]) {
  const summary = [
    `账户: ${failure.accountName}`,
    `站点: ${failure.siteType}`,
    `归属: ${failure.ownerName || '-'}`,
    `时间: ${formatDateTime(failure.createdAt)}`,
    `消息: ${failure.message || '-'}`,
  ].join('\n')

  try {
    await navigator.clipboard.writeText(summary)
    showToast('失败摘要已复制', 'success')
  } catch {
    showToast('复制失败，请手动选择消息内容', 'error')
  }
}

onMounted(() => {
  setDefaultRange()
  if (props.isAdmin) {
    loadUsers()
  }
  loadStatistics()
})

watch(selectedUserId, () => {
  loadStatistics()
})

watch(() => props.isAdmin, (isAdmin) => {
  if (isAdmin) {
    loadUsers()
    return
  }
  selectedUserId.value = ''
})
</script>

<style scoped>
.statistics-panel {
  max-width: 1200px;
  margin: 0 auto;
  padding: clamp(1rem, 2.5vw, 2.25rem) 0 3rem;
  color: var(--text-strong);
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 24px;
  flex-wrap: wrap;
  gap: 16px;
}

.panel-header h2 {
  color: var(--text-strong);
}

.panel-subtitle {
  color: var(--text-muted);
  font-size: 0.9rem;
  margin-top: 0.25rem;
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 12px;
}

.date-range {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
}

.date-separator {
  color: var(--text);
}

.date-input,
.user-filter {
  padding: 8px 12px;
  border: 1px solid var(--border-input);
  border-radius: 6px;
  background: var(--bg-well);
  color: #fff;
  font-size: 14px;
}

.user-filter {
  min-width: 160px;
}

button {
  border: 0;
  border-radius: 6px;
  padding: 8px 12px;
  background: var(--border-input);
  color: #fff;
  cursor: pointer;
}

button.primary {
  background: var(--accent);
}

button:hover:not(:disabled) {
  filter: brightness(1.08);
}

button:disabled,
.date-input:disabled,
.user-filter:disabled {
  cursor: not-allowed;
  opacity: 0.65;
}

.validation-box {
  background: rgba(239, 68, 68, 0.08);
  border: 1px solid rgba(239, 68, 68, 0.45);
  color: #fca5a5;
  border-radius: var(--radius);
  padding: 0.85rem 1rem;
  margin-bottom: 1rem;
}

.overview-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 16px;
  margin-bottom: 32px;
}

.stat-card {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 20px;
  display: flex;
  align-items: center;
  gap: 16px;
  box-shadow: var(--shadow-card);
}

.stat-card.warning {
  border-color: rgba(239, 68, 68, 0.45);
}

.stat-mark {
  min-width: 48px;
  height: 48px;
  border-radius: var(--radius);
  background: var(--bg-well);
  border: 1px solid var(--border);
  display: grid;
  place-items: center;
  color: #93c5fd;
  font-size: 0.8rem;
  font-weight: 600;
}

.stat-info {
  flex: 1;
}

.stat-value {
  font-size: 28px;
  font-weight: 600;
  color: var(--text-strong);
  margin-bottom: 4px;
}

.stat-label {
  font-size: 14px;
  color: var(--text-faint);
  margin-bottom: 4px;
}

.stat-sub {
  font-size: 12px;
  color: var(--text-muted);
}

.insight-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 16px;
  margin-bottom: 24px;
}

.insight-item {
  background: var(--bg-well);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 1rem;
  display: grid;
  gap: 0.35rem;
}

.insight-item span {
  color: var(--text-muted);
  font-size: 0.85rem;
}

.insight-item strong {
  color: var(--text-strong);
  font-size: 1rem;
  overflow-wrap: anywhere;
}

.chart-section, .table-section, .info-section, .failure-section {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 24px;
  margin-bottom: 24px;
  color: var(--text-strong);
  box-shadow: var(--shadow-card);
}

.table-section {
  overflow-x: auto;
}

.chart-section h3, .table-section h3, .info-section h3, .failure-section h3 {
  margin: 0 0 16px 0;
  font-size: 18px;
  color: var(--text-strong);
}

.chart-legend {
  display: flex;
  gap: 16px;
  margin-bottom: 16px;
  font-size: 14px;
  color: var(--text-faint);
}

.legend-item {
  display: flex;
  align-items: center;
  gap: 6px;
}

.dot {
  width: 12px;
  height: 12px;
  border-radius: 2px;
}

.dot.success { background: #4caf50; }
.dot.failed { background: #f44336; }
.dot.already { background: #ff9800; }

.bar-chart {
  display: flex;
  gap: 12px;
  align-items: flex-end;
  height: 200px;
  padding: 16px;
  background: var(--bg-well);
  border: 1px solid var(--border);
  border-radius: 6px;
  overflow-x: auto;
}

.bar-group {
  display: flex;
  flex-direction: column;
  align-items: center;
  min-width: 40px;
}

.bar-stack {
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  align-items: center;
  height: 140px;
  width: 32px;
  gap: 2px;
}

.bar {
  width: 100%;
  border-radius: 2px 2px 0 0;
  transition: opacity 0.2s;
  cursor: default;
  min-height: 2px;
}

.bar:hover {
  opacity: 0.8;
}

.bar-group:focus-visible {
  outline: 2px solid var(--focus-ring);
  outline-offset: 4px;
  border-radius: 4px;
}

.bar.success { background: #4caf50; }
.bar.failed { background: #f44336; }
.bar.already { background: #ff9800; }

.bar-label {
  font-size: 11px;
  color: var(--text-muted);
  margin-top: 8px;
  white-space: nowrap;
}

.bar-value {
  font-size: 12px;
  font-weight: 600;
  color: #e5e7eb;
  margin-top: 4px;
}

.bar-rate {
  font-size: 11px;
  color: var(--text-muted);
}

.stats-table {
  width: 100%;
  min-width: 720px;
  border-collapse: collapse;
  color: #e5e7eb;
}

.stats-table th {
  background: var(--bg-app);
  padding: 12px;
  text-align: left;
  font-weight: 600;
  font-size: 14px;
  color: var(--text-strong);
  border-bottom: 1px solid var(--border-strong);
}

.stats-table td {
  padding: 12px;
  border-bottom: 1px solid var(--border);
  font-size: 14px;
  color: var(--text-faint);
}

.stats-table tbody tr:hover {
  background: var(--bg-app);
}

.success-text {
  color: #34d399;
  font-weight: 600;
}

.failed-text {
  color: #f87171;
  font-weight: 600;
}

.badge {
  display: inline-block;
  padding: 4px 8px;
  border-radius: 4px;
  background: #1e3a8a;
  color: #dbeafe;
  font-size: 12px;
  font-weight: 600;
}

.rate-badge {
  display: inline-block;
  padding: 4px 8px;
  border-radius: 4px;
  font-size: 13px;
  font-weight: 600;
}

.rate-excellent { background: var(--success-soft); color: #34d399; }
.rate-good { background: rgba(245, 158, 11, 0.18); color: #fbbf24; }
.rate-fair { background: rgba(249, 115, 22, 0.18); color: #fb923c; }
.rate-poor { background: var(--danger-soft); color: #f87171; }

.failure-list {
  display: grid;
  gap: 0.75rem;
}

.failure-item {
  background: var(--bg-well);
  border: 1px solid var(--border);
  border-left: 4px solid var(--danger);
  border-radius: var(--radius);
  padding: 0.95rem;
  display: flex;
  justify-content: space-between;
  gap: 1rem;
}

.failure-main {
  min-width: 0;
}

.failure-title {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex-wrap: wrap;
  margin-bottom: 0.35rem;
}

.failure-title strong {
  color: var(--text-strong);
  overflow-wrap: anywhere;
}

.owner-tag {
  color: var(--text-faint);
  background: #1e293b;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-pill);
  padding: 0.15rem 0.5rem;
  font-size: 0.75rem;
}

.failure-message {
  color: var(--text-faint);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 760px;
}

.info-text {
  font-size: 16px;
  margin-bottom: 12px;
  color: var(--text-faint);
}

.info-text strong {
  font-size: 20px;
  color: #93c5fd;
}

.empty {
  text-align: center;
  padding: 40px;
  color: var(--text-muted);
}

.muted {
  color: var(--text-muted);
  font-size: 14px;
}

@media (max-width: 768px) {
  .statistics-panel { padding: 1rem; }
  .toolbar { justify-content: flex-start; }
  .date-range,
  .date-input,
  .user-filter { width: 100%; }
  .date-range { flex-direction: column; align-items: stretch; }
  .date-range .date-input { width: 100%; }
  .date-separator { text-align: center; }
  .date-range button { flex: 1; width: 100%; }
  .overview-grid { grid-template-columns: 1fr; }
  .insight-grid { grid-template-columns: 1fr; }
  .failure-item { display: grid; }
  .failure-item button { width: 100%; }
  .failure-message { white-space: normal; max-width: none; }
}
</style>
