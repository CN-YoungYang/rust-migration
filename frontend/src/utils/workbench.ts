export type WorkbenchTodayResult =
  | 'not_checked'
  | 'success'
  | 'already_checked'
  | 'failed'
  | 'pending'
  | 'disabled'

export interface WorkbenchAccountLike {
  id: string
  enabled?: boolean
  selectable?: boolean
  todayResult?: WorkbenchTodayResult | string | null
  retryEnabled?: boolean
}

export interface WorkbenchFilters {
  keyword: string
  siteType: string
  userId: string
  enabled: string
  status: string
  todayResult: string
  balanceWarning: string
}

export interface WorkbenchQueryOptions {
  isAdmin: boolean
  page: number
  limit: number
}

export interface BatchItemLike {
  accountId: string
  status: string
}

const TERMINAL_BATCH_STATUSES = new Set(['completed', 'partial_failed', 'failed'])

export function isTerminalBatchStatus(status: string | null | undefined): boolean {
  return status != null && TERMINAL_BATCH_STATUSES.has(status.toLowerCase())
}

export function buildWorkbenchQuery(
  filters: WorkbenchFilters,
  options: WorkbenchQueryOptions,
): string {
  const params = new URLSearchParams()
  params.set('limit', String(Math.max(1, Math.min(100, Math.round(options.limit)))))
  params.set('offset', String(Math.max(0, (Math.max(1, Math.round(options.page)) - 1) * Math.max(1, Math.round(options.limit)))))

  if (options.isAdmin && filters.userId) params.set('userId', filters.userId)
  if (filters.keyword.trim()) params.set('keyword', filters.keyword.trim())
  if (filters.siteType) params.set('siteType', filters.siteType)
  if (filters.enabled) params.set('enabled', filters.enabled)
  if (filters.status) params.set('status', filters.status)
  if (filters.todayResult) params.set('todayResult', filters.todayResult)
  if (filters.balanceWarning) params.set('balanceWarning', filters.balanceWarning)
  return params.toString()
}

export function selectableAccountIds(accounts: readonly WorkbenchAccountLike[]): string[] {
  return accounts
    .filter((account) => account.enabled !== false && account.selectable !== false)
    .map((account) => account.id)
}

export function retryableFailedAccountIds(
  items: readonly BatchItemLike[],
  accounts: readonly WorkbenchAccountLike[],
): string[] {
  const accountMap = new Map(accounts.map((account) => [account.id, account]))
  const ids: string[] = []
  const seen = new Set<string>()
  for (const item of items) {
    if (item.status.toLowerCase() !== 'failed' || seen.has(item.accountId)) continue
    const account = accountMap.get(item.accountId)
    if (account?.enabled === false || account?.retryEnabled === false) continue
    seen.add(item.accountId)
    ids.push(item.accountId)
  }
  return ids
}

export function mergeTrackedBatchIds(
  current: readonly string[],
  incoming: readonly string[],
  max = 20,
): string[] {
  const result: string[] = []
  const seen = new Set<string>()
  for (const value of [...incoming, ...current]) {
    const id = value.trim()
    if (!id || seen.has(id)) continue
    seen.add(id)
    result.push(id)
    if (result.length >= Math.max(1, max)) break
  }
  return result
}

export function batchStatusText(status: string | null | undefined): string {
  const labels: Record<string, string> = {
    pending: '等待执行',
    running: '执行中',
    completed: '已完成',
    partial_failed: '部分失败',
    failed: '全部失败',
  }
  if (!status) return '未知状态'
  return labels[status.toLowerCase()] || status
}

export function batchStatusTagType(
  status: string | null | undefined,
): 'default' | 'success' | 'warning' | 'error' {
  const normalized = status?.toLowerCase()
  if (normalized === 'completed') return 'success'
  if (normalized === 'partial_failed') return 'warning'
  if (normalized === 'failed') return 'error'
  if (normalized === 'pending' || normalized === 'running') return 'warning'
  return 'default'
}
