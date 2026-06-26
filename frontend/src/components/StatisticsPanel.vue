<template>
  <section class="statistics-panel">
    <div class="panel-header">
      <h2>📊 数据统计</h2>
      <div class="toolbar">
        <select
          v-if="isAdmin"
          v-model="selectedUserId"
          class="user-filter"
          :disabled="usersLoading || loading"
        >
          <option value="">全部用户</option>
          <option v-if="usersLoading" disabled>加载中...</option>
          <option v-for="u in allUsers" :key="u.id" :value="u.id">
            {{ u.username }}{{ u.id === currentUser?.id ? '（我）' : '' }}
          </option>
        </select>
        <div class="date-range">
          <input v-model="startDate" type="date" class="date-input" :disabled="loading" />
          <span class="date-separator">至</span>
          <input v-model="endDate" type="date" class="date-input" :disabled="loading" />
          <button class="primary" @click="loadStatistics" :disabled="loading">
            {{ loading ? '查询中...' : '查询' }}
          </button>
          <button @click="applyDefaultRange" :disabled="loading">最近30天</button>
        </div>
      </div>
    </div>

    <p v-if="loading" class="empty">加载中...</p>

    <div v-if="!loading && statistics" class="stats-content">
      <!-- 概览卡片 -->
      <div class="overview-grid">
        <div class="stat-card">
          <div class="stat-icon">📦</div>
          <div class="stat-info">
            <div class="stat-value">{{ statistics.overview.totalAccounts }}</div>
            <div class="stat-label">总账户数</div>
            <div class="stat-sub">已启用：{{ statistics.overview.enabledAccounts }}</div>
          </div>
        </div>

        <div class="stat-card">
          <div class="stat-icon">✅</div>
          <div class="stat-info">
            <div class="stat-value">{{ statistics.overview.todaySuccess }}</div>
            <div class="stat-label">今日成功</div>
            <div class="stat-sub">失败：{{ statistics.overview.todayFailed }}</div>
          </div>
        </div>

        <div class="stat-card">
          <div class="stat-icon">📈</div>
          <div class="stat-info">
            <div class="stat-value">{{ statistics.overview.successRate.toFixed(1) }}%</div>
            <div class="stat-label">总成功率</div>
            <div class="stat-sub">总计：{{ statistics.overview.totalRuns }} 次</div>
          </div>
        </div>

        <div class="stat-card">
          <div class="stat-icon">💰</div>
          <div class="stat-info">
            <div class="stat-value">${{ statistics.overview.totalBalance.toFixed(2) }}</div>
            <div class="stat-label">总余额</div>
            <div class="stat-sub">已启用账户</div>
          </div>
        </div>
      </div>

      <!-- 每日趋势图表 -->
      <div class="chart-section">
        <h3>📅 每日签到趋势</h3>
        <div v-if="statistics.dailyTrend.length > 0" class="chart-container">
          <div class="chart-legend">
            <span class="legend-item"><span class="dot success"></span>成功</span>
            <span class="legend-item"><span class="dot failed"></span>失败</span>
            <span class="legend-item"><span class="dot already"></span>已签到</span>
          </div>
          <div class="bar-chart">
            <div v-for="day in statistics.dailyTrend" :key="day.date" class="bar-group">
              <div class="bar-stack">
                <div
                  class="bar success"
                  :style="{ height: getBarHeight(day.success, day.total) + '%' }"
                  :title="`成功：${day.success}`"
                ></div>
                <div
                  class="bar already"
                  :style="{ height: getBarHeight(day.alreadyChecked, day.total) + '%' }"
                  :title="`已签到：${day.alreadyChecked}`"
                ></div>
                <div
                  class="bar failed"
                  :style="{ height: getBarHeight(day.failed, day.total) + '%' }"
                  :title="`失败：${day.failed}`"
                ></div>
              </div>
              <div class="bar-label">{{ formatDate(day.date) }}</div>
              <div class="bar-value">{{ day.total }}</div>
            </div>
          </div>
        </div>
        <p v-else class="empty">所选时间范围内无签到记录</p>
      </div>

      <!-- 站点统计表格 -->
      <div class="table-section">
        <h3>🌐 站点统计</h3>
        <table v-if="statistics.siteStats.length > 0" class="stats-table">
          <thead>
            <tr>
              <th>站点类型</th>
              <th>账户数</th>
              <th>总签到</th>
              <th>成功</th>
              <th>失败</th>
              <th>成功率</th>
              <th>平均耗时</th>
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
        <p v-else class="empty">暂无站点统计</p>
      </div>

      <!-- 余额趋势（当前为快照，未来可扩展） -->
      <div class="info-section">
        <h3>💵 余额信息</h3>
        <p class="info-text">
          当前总余额：<strong>${{ statistics.overview.totalBalance.toFixed(2) }}</strong>
          （{{ (statistics.overview.totalBalance * 500000).toFixed(0) }} quota）
        </p>
        <p class="muted">💡 提示：余额基于最后一次刷新时间，可能不是实时数据</p>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
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

// 设置默认时间范围（最近30天）
function setDefaultRange() {
  const today = new Date()
  const thirtyDaysAgo = new Date(today)
  thirtyDaysAgo.setDate(today.getDate() - 29)

  endDate.value = formatDateInput(today)
  startDate.value = formatDateInput(thirtyDaysAgo)
}

function applyDefaultRange() {
  setDefaultRange()
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

function getRateClass(rate: number): string {
  if (rate >= 90) return 'rate-excellent'
  if (rate >= 70) return 'rate-good'
  if (rate >= 50) return 'rate-fair'
  return 'rate-poor'
}

async function loadStatistics() {
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
  padding: 20px;
  color: #f8fafc;
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
  color: #f8fafc;
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
  color: #d1d5db;
}

.date-input,
.user-filter {
  padding: 8px 12px;
  border: 1px solid #374151;
  border-radius: 4px;
  background: #0b1220;
  color: #fff;
  font-size: 14px;
}

.user-filter {
  min-width: 160px;
}

button {
  border: 0;
  border-radius: 4px;
  padding: 8px 12px;
  background: #374151;
  color: #fff;
  cursor: pointer;
}

button.primary {
  background: #0070f3;
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

.overview-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 16px;
  margin-bottom: 32px;
}

.stat-card {
  background: white;
  border: 1px solid #e7e3da;
  border-radius: 8px;
  padding: 20px;
  display: flex;
  align-items: center;
  gap: 16px;
}

.stat-icon {
  font-size: 32px;
}

.stat-info {
  flex: 1;
}

.stat-value {
  font-size: 28px;
  font-weight: 600;
  color: #1e2227;
  margin-bottom: 4px;
}

.stat-label {
  font-size: 14px;
  color: #6b7077;
  margin-bottom: 4px;
}

.stat-sub {
  font-size: 12px;
  color: #999;
}

.chart-section, .table-section, .info-section {
  background: white;
  border: 1px solid #e7e3da;
  border-radius: 8px;
  padding: 24px;
  margin-bottom: 24px;
  color: #1e2227;
}

.table-section {
  overflow-x: auto;
}

.chart-section h3, .table-section h3, .info-section h3 {
  margin: 0 0 16px 0;
  font-size: 18px;
  color: #1e2227;
}

.chart-legend {
  display: flex;
  gap: 16px;
  margin-bottom: 16px;
  font-size: 14px;
  color: #3f4650;
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
  background: #fafafa;
  border-radius: 4px;
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
  cursor: pointer;
  min-height: 2px;
}

.bar:hover {
  opacity: 0.8;
}

.bar.success { background: #4caf50; }
.bar.failed { background: #f44336; }
.bar.already { background: #ff9800; }

.bar-label {
  font-size: 11px;
  color: #666;
  margin-top: 8px;
  white-space: nowrap;
}

.bar-value {
  font-size: 12px;
  font-weight: 600;
  color: #333;
  margin-top: 4px;
}

.stats-table {
  width: 100%;
  min-width: 720px;
  border-collapse: collapse;
  color: #1e2227;
}

.stats-table th {
  background: #f7f5f1;
  padding: 12px;
  text-align: left;
  font-weight: 600;
  font-size: 14px;
  color: #1e2227;
  border-bottom: 2px solid #e7e3da;
}

.stats-table td {
  padding: 12px;
  border-bottom: 1px solid #f0f0f0;
  font-size: 14px;
  color: #2f363e;
}

.stats-table tbody tr:hover {
  background: #fafafa;
}

.success-text {
  color: #2e7d32;
  font-weight: 600;
}

.failed-text {
  color: #c62828;
  font-weight: 600;
}

.badge {
  display: inline-block;
  padding: 4px 8px;
  border-radius: 4px;
  background: #eef2ff;
  color: #1f3a8a;
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

.rate-excellent { background: #e8f5e9; color: #2e7d32; }
.rate-good { background: #fff9c4; color: #f57f17; }
.rate-fair { background: #ffe0b2; color: #e65100; }
.rate-poor { background: #ffebee; color: #c62828; }

.info-text {
  font-size: 16px;
  margin-bottom: 12px;
  color: #2f363e;
}

.info-text strong {
  font-size: 20px;
  color: #3c5a78;
}

.empty {
  text-align: center;
  padding: 40px;
  color: #999;
}

.muted {
  color: #6b7077;
  font-size: 14px;
}
</style>
