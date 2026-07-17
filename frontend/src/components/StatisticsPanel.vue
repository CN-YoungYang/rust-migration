<template>
  <section ref="panelRoot" class="statistics-panel">
    <div class="panel-header">
      <div>
        <h2>数据统计</h2>
        <p class="panel-subtitle">{{ resultUserName }} · {{ resultRangeLabel }}</p>
      </div>
      <div class="toolbar">
        <label v-if="isAdmin" class="filter-field">
          <span>统计用户</span>
          <select v-model="selectedUserId" class="user-filter" :disabled="usersLoading || loading">
            <option value="">全部用户</option>
            <option v-if="usersLoading" disabled>加载中...</option>
            <option v-for="u in allUsers" :key="u.id" :value="u.id">
              {{ u.username }}{{ u.id === currentUser?.id ? '（我）' : '' }}
            </option>
          </select>
        </label>
        <div class="date-range" role="group" aria-label="统计日期范围">
          <label class="filter-field"><span>开始日期</span><input v-model="startDate" type="date" class="date-input" :disabled="loading" /></label>
          <span class="date-separator">至</span>
          <label class="filter-field"><span>结束日期</span><input v-model="endDate" type="date" class="date-input" :disabled="loading" /></label>
          <button class="primary" @click="loadStatistics" :disabled="loading || dateRangeInvalid || dateRangeTooLong">
            {{ loading ? '查询中...' : '查询' }}
          </button>
          <button v-for="days in [7, 30, 90]" :key="days" @click="applyRange(days)" :disabled="loading" :aria-pressed="isActiveRange(days)" :class="{ selected: isActiveRange(days) }">{{ days }}天</button>
        </div>
      </div>
    </div>

    <div v-if="dateRangeInvalid" class="validation-box" role="alert">开始日期不能晚于结束日期。</div>
    <div v-else-if="dateRangeTooLong" class="validation-box" role="alert">统计查询范围不能超过180天。</div>

    <p v-if="loading && !statistics" class="empty" role="status" aria-live="polite">正在加载统计数据...</p>
    <div v-if="loadError" class="load-error" role="alert">
      <span>{{ loadError }}</span>
      <button @click="loadStatistics" :disabled="loading">重新查询</button>
    </div>

    <div v-if="statistics" class="stats-content" :aria-busy="loading">
      <p v-if="loading" class="refresh-status" role="status" aria-live="polite">正在更新，当前仍显示上次结果。</p>
      <dl class="summary-strip" aria-label="统计概览">
        <div>
          <dt>今日执行</dt>
          <dd>{{ statistics.overview.todayTotal }}<small>次</small></dd>
          <p>成功 {{ statistics.overview.todaySuccess }} · 已签到 {{ statistics.overview.todayAlreadyChecked }} · 等待 {{ statistics.overview.todayPending }} · 失败 {{ statistics.overview.todayFailed }}</p>
        </div>
        <div :class="{ warning: statistics.overview.todayFailed > 0 }">
          <dt>区间成功率</dt>
          <dd>{{ statistics.overview.completedRuns > 0 ? statistics.overview.successRate.toFixed(1) : '—' }}<small v-if="statistics.overview.completedRuns > 0">%</small></dd>
          <p>按已完成记录计算 · 总执行 {{ statistics.overview.totalRuns }} 次</p>
        </div>
        <div>
          <dt>启用账户</dt>
          <dd>{{ statistics.overview.enabledAccounts }}<small>/ {{ statistics.overview.totalAccounts }}</small></dd>
          <p>启用率 {{ enabledRatio }}%</p>
        </div>
        <div>
          <dt>当前总余额</dt>
          <dd><small>$</small>{{ statistics.overview.totalBalance.toFixed(2) }}</dd>
          <p>基于账户最后刷新结果</p>
        </div>
      </dl>

      <div class="chart-section">
        <div class="section-heading">
          <div>
            <h3>每日执行量</h3>
            <p>统一按区间内最大单日执行量绘制，柱高可直接跨日期比较。</p>
          </div>
          <div class="chart-scale"><span>最高单日</span><strong>{{ maxDailyTotal }} 次</strong></div>
        </div>
        <div v-if="statistics.dailyTrend.length > 0" class="chart-container" role="group" aria-label="每日签到趋势图">
          <div class="chart-legend">
            <span class="legend-item"><span class="dot success"></span>成功</span>
            <span class="legend-item"><span class="dot already"></span>已签到</span>
            <span class="legend-item"><span class="dot pending"></span>等待中</span>
            <span class="legend-item"><span class="dot failed"></span>失败</span>
          </div>
          <div class="plot-scroll" aria-label="每日执行量图表，可横向滚动">
            <div class="bar-chart" role="group" aria-label="每日签到趋势数据">
              <button
                v-for="day in dailyTrendSeries"
                :key="day.date"
                type="button"
                class="bar-group"
                :class="{ selected: selectedTrendDay?.date === day.date }"
                :data-trend-date="day.date"
                :tabindex="selectedTrendDay?.date === day.date ? 0 : -1"
                :aria-label="trendAriaLabel(day)"
                :aria-pressed="selectedTrendDay?.date === day.date"
                @focus="activeTrendDate = day.date"
                @click="activeTrendDate = day.date"
                @keydown.left.prevent="moveTrendFocus(day.date, -1)"
                @keydown.right.prevent="moveTrendFocus(day.date, 1)"
                @keydown.home.prevent="moveTrendFocus(day.date, -dailyTrendSeries.length)"
                @keydown.end.prevent="moveTrendFocus(day.date, dailyTrendSeries.length)"
              >
                <div class="bar-stack" aria-hidden="true">
                  <div v-if="day.success > 0" class="bar success" :style="{ height: getTrendHeight(day.success) + '%' }"></div>
                  <div v-if="day.alreadyChecked > 0" class="bar already" :style="{ height: getTrendHeight(day.alreadyChecked) + '%' }"></div>
                  <div v-if="day.pending > 0" class="bar pending" :style="{ height: getTrendHeight(day.pending) + '%' }"></div>
                  <div v-if="day.failed > 0" class="bar failed" :style="{ height: getTrendHeight(day.failed) + '%' }"></div>
                </div>
                <span class="bar-label">{{ formatDate(day.date) }}</span>
                <span class="bar-value">{{ day.total }}</span>
                <span class="bar-rate">{{ completedCount(day) > 0 ? `${day.successRate.toFixed(0)}%` : '—' }}</span>
              </button>
            </div>
          </div>
          <dl v-if="selectedTrendDay" class="trend-detail">
            <div><dt>日期</dt><dd>{{ selectedTrendDay.date }}</dd></div>
            <div><dt>总执行</dt><dd>{{ selectedTrendDay.total }}</dd></div>
            <div><dt>成功</dt><dd>{{ selectedTrendDay.success }}</dd></div>
            <div><dt>已签到</dt><dd>{{ selectedTrendDay.alreadyChecked }}</dd></div>
            <div><dt>等待中</dt><dd>{{ selectedTrendDay.pending }}</dd></div>
            <div><dt>失败</dt><dd>{{ selectedTrendDay.failed }}</dd></div>
            <div><dt>成功率</dt><dd>{{ completedCount(selectedTrendDay) > 0 ? `${selectedTrendDay.successRate.toFixed(1)}%` : '—' }}</dd></div>
          </dl>
        </div>
        <div v-else class="empty" role="status">
          <p>所选时间范围内无签到记录。</p>
          <button @click="applyRange(30)" :disabled="loading">查看最近30天</button>
        </div>
      </div>

      <div class="table-section">
        <h3>站点统计</h3>
        <table v-if="statistics.siteStats.length > 0" class="stats-table">
          <caption class="sr-only">按站点类型汇总的账户数、签到结果、成功率和平均耗时</caption>
          <thead>
            <tr>
              <th scope="col">站点类型</th>
              <th scope="col">账户数</th>
              <th scope="col">总签到</th>
              <th scope="col">成功</th>
              <th scope="col">已签到</th>
              <th scope="col">失败</th>
              <th scope="col">等待中</th>
              <th scope="col">成功率</th>
              <th scope="col">平均耗时</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="site in statistics.siteStats" :key="site.siteType">
              <td data-label="站点类型"><span class="badge">{{ site.siteType }}</span></td>
              <td data-label="账户数">{{ site.accountCount }}</td>
              <td data-label="总签到">{{ site.totalRuns }}</td>
              <td data-label="成功" class="success-text">{{ site.success }}</td>
              <td data-label="已签到">{{ site.alreadyChecked }}</td>
              <td data-label="失败" class="failed-text">{{ site.failed }}</td>
              <td data-label="等待中">{{ site.pending }}</td>
              <td data-label="成功率">
                <span class="rate-badge" :class="completedCount(site) > 0 ? getRateClass(site.successRate) : 'rate-empty'">
                  {{ completedCount(site) > 0 ? `${site.successRate.toFixed(1)}%` : '—' }}
                </span>
              </td>
              <td data-label="平均耗时">{{ site.avgDuration == null ? '—' : `${site.avgDuration.toFixed(0)}ms` }}</td>
            </tr>
          </tbody>
        </table>
        <p v-else class="empty" role="status">暂无站点统计</p>
      </div>

      <div class="failure-section">
        <div class="section-heading">
          <div>
            <h3>异常与风险</h3>
            <p>最近失败不受当前日期范围限制，用于快速定位仍需处理的问题。</p>
          </div>
          <div class="risk-summary"><span>风险站点</span><strong>{{ highestRiskSite }}</strong></div>
        </div>
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

    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, ref, nextTick, onMounted, watch } from 'vue'
import { apiUrl, authHeaders, request, responseData } from '../utils/api'
import { showToast } from '../utils/toast'
import type { CurrentUser } from '../types'
import { useUsers } from '../composables/useUsers'

interface Statistics {
  overview: {
    totalAccounts: number
    enabledAccounts: number
    todaySuccess: number
    todayAlreadyChecked: number
    todayFailed: number
    todayPending: number
    todayTotal: number
    totalRuns: number
    completedRuns: number
    successRate: number
    totalBalance: number
  }
  dailyTrend: Array<{
    date: string
    success: number
    failed: number
    alreadyChecked: number
    pending: number
    total: number
    successRate: number
  }>
  siteStats: Array<{
    siteType: string
    accountCount: number
    totalRuns: number
    success: number
    alreadyChecked: number
    failed: number
    pending: number
    successRate: number
    avgDuration: number | null
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
const activeTrendDate = ref('')
const loadError = ref('')
const panelRoot = ref<HTMLElement | null>(null)
const appliedQuery = ref({ startDate: '', endDate: '', userName: '' })
let requestSeq = 0

const selectedUserName = computed(() => {
  if (!props.isAdmin) return props.currentUser?.username ? `${props.currentUser.username}（我的数据）` : '我的数据'
  if (!selectedUserId.value) return '全部用户'
  return allUsers.value.find((user) => user.id === selectedUserId.value)?.username || '指定用户'
})

const inputRangeLabel = computed(() => {
  if (!startDate.value || !endDate.value) return '默认时间范围'
  return `${startDate.value} 至 ${endDate.value}`
})

const resultUserName = computed(() => statistics.value ? appliedQuery.value.userName : selectedUserName.value)
const resultRangeLabel = computed(() => {
  if (!statistics.value) return inputRangeLabel.value
  return `${appliedQuery.value.startDate} 至 ${appliedQuery.value.endDate}`
})

const dateRangeInvalid = computed(() => {
  return Boolean(startDate.value && endDate.value && startDate.value > endDate.value)
})

const dateRangeTooLong = computed(() => {
  if (!startDate.value || !endDate.value || dateRangeInvalid.value) return false
  return inclusiveDayCount(startDate.value, endDate.value) > 180
})

const enabledRatio = computed(() => {
  if (!statistics.value || statistics.value.overview.totalAccounts === 0) return '0.0'
  return (statistics.value.overview.enabledAccounts / statistics.value.overview.totalAccounts * 100).toFixed(1)
})

const maxDailyTotal = computed(() => {
  return Math.max(...dailyTrendSeries.value.map((day) => day.total), 0)
})

const dailyTrendSeries = computed<Statistics['dailyTrend']>(() => {
  if (!statistics.value || !appliedQuery.value.startDate || !appliedQuery.value.endDate) return []
  const byDate = new Map(statistics.value.dailyTrend.map((day) => [day.date, day]))
  const series: Statistics['dailyTrend'] = []
  const cursor = new Date(`${appliedQuery.value.startDate}T00:00:00`)
  const end = new Date(`${appliedQuery.value.endDate}T00:00:00`)

  while (cursor <= end) {
    const date = formatDateInput(cursor)
    series.push(byDate.get(date) || {
      date,
      success: 0,
      failed: 0,
      alreadyChecked: 0,
      pending: 0,
      total: 0,
      successRate: 0,
    })
    cursor.setDate(cursor.getDate() + 1)
  }
  return series
})

const selectedTrendDay = computed(() => {
  const selected = dailyTrendSeries.value.find((day) => day.date === activeTrendDate.value)
  if (selected) return selected
  return [...dailyTrendSeries.value].reverse().find((day) => day.total > 0) || dailyTrendSeries.value.at(-1) || null
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

function isActiveRange(days: number): boolean {
  if (!startDate.value || !endDate.value) return false
  const start = new Date(`${startDate.value}T00:00:00`)
  const end = new Date(`${endDate.value}T00:00:00`)
  const today = formatDateInput(new Date())
  return endDate.value === today && Math.round((end.getTime() - start.getTime()) / 86400000) + 1 === days
}

function inclusiveDayCount(start: string, end: string): number {
  const startDateTime = new Date(`${start}T00:00:00`)
  const endDateTime = new Date(`${end}T00:00:00`)
  return Math.round((endDateTime.getTime() - startDateTime.getTime()) / 86400000) + 1
}

function formatDateInput(date: Date): string {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

function formatDate(dateStr: string): string {
  const [year, month, day] = dateStr.split('-')
  const crossesYear = appliedQuery.value.startDate.slice(0, 4) !== appliedQuery.value.endDate.slice(0, 4)
  return crossesYear ? `${year}/${month}/${day}` : `${month}/${day}`
}

function getTrendHeight(value: number): number {
  if (maxDailyTotal.value === 0) return 0
  return (value / maxDailyTotal.value) * 100
}

function completedCount(item: { success: number; alreadyChecked: number; failed: number }): number {
  return item.success + item.alreadyChecked + item.failed
}

function moveTrendFocus(date: string, offset: number) {
  const currentIndex = dailyTrendSeries.value.findIndex((day) => day.date === date)
  if (currentIndex < 0) return
  const nextIndex = Math.max(0, Math.min(dailyTrendSeries.value.length - 1, currentIndex + offset))
  const nextDate = dailyTrendSeries.value[nextIndex]?.date
  if (!nextDate) return
  activeTrendDate.value = nextDate
  void nextTick(() => {
    panelRoot.value?.querySelector<HTMLElement>(`[data-trend-date="${nextDate}"]`)?.focus()
  })
}

function trendAriaLabel(day: Statistics['dailyTrend'][number]): string {
  const rate = completedCount(day) > 0 ? `${day.successRate.toFixed(0)}%` : '无样本'
  return `${formatDate(day.date)}：总计 ${day.total} 次，成功 ${day.success} 次，已签到 ${day.alreadyChecked} 次，等待中 ${day.pending} 次，失败 ${day.failed} 次，成功率 ${rate}`
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
  if (dateRangeTooLong.value) {
    showToast('统计查询范围不能超过180天', 'error')
    return
  }
  const requestedQuery = {
    startDate: startDate.value,
    endDate: endDate.value,
    userName: selectedUserName.value,
  }
  const seq = ++requestSeq
  loadError.value = ''
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
      appliedQuery.value = requestedQuery
      activeTrendDate.value = data.dailyTrend.at(-1)?.date || requestedQuery.endDate
    }
  } catch (error) {
    if (seq === requestSeq) {
      loadError.value = error instanceof Error ? error.message : '加载统计数据失败'
      showToast(loadError.value, 'error')
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

<style scoped src="./StatisticsPanel.css"></style>
