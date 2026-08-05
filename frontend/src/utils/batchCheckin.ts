/**
 * 前端批量签到工具：逐账户请求所需的状态判断与随机化助手。
 * 语义对齐后端 `services/checkin/runner.rs` 的 `skip_reason_for_batch` /
 * `random_delay_secs`，便于前端复刻批量的「跳过 / 随机延迟 / 打乱顺序」行为，
 * 同时避免单个长 HTTP 请求被反向代理 / Cloudflare 超时整批截断。
 */

export interface BatchSkipAccount {
  enabled?: boolean
  lastRunAt?: string | null
  lastStatus?: string | null
  retryEnabled?: boolean
  todayRuns?: number
}

export interface BatchSkipSettings {
  retryEnabled?: boolean
  maxAttemptsPerDay?: number
}

/** 判断 ISO 时间串是否落在本地时区的“今天”（与后端 last_run → Local 取日界一致）。 */
export function isSameLocalDay(iso: string | null | undefined): boolean {
  if (!iso) return false
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return false
  const now = new Date()
  return (
    date.getFullYear() === now.getFullYear() &&
    date.getMonth() === now.getMonth() &&
    date.getDate() === now.getDate()
  )
}

/**
 * 批量签到前的跳过判断（对齐后端 skip_reason_for_batch）：
 * 已禁用 / 今日已签（success、already_checked）/ 今日尝试过且关闭重试 /
 * （能读到设置时）已达每日尝试上限。
 * 返回中文跳过原因；返回 null 表示需要执行。
 */
export function batchSkipReason(
  account: BatchSkipAccount | undefined,
  settings: BatchSkipSettings | null,
): string | null {
  if (!account) return null
  if (account.enabled === false) return '账户已禁用'

  if (isSameLocalDay(account.lastRunAt)) {
    if (account.lastStatus === 'success' || account.lastStatus === 'already_checked') {
      return '今日已签到'
    }
    if (settings?.retryEnabled === false || account.retryEnabled === false) {
      return '重试已关闭'
    }
  }

  if (settings?.maxAttemptsPerDay != null && (account.todayRuns ?? 0) >= settings.maxAttemptsPerDay) {
    return `已达今日尝试上限（${settings.maxAttemptsPerDay}）`
  }
  return null
}

/** 随机延迟秒数（对齐后端 random_delay_secs）。min=0 且 max=0 时返回 0（不延迟）。 */
export function randomDelaySecs(min: number, max: number): number {
  const lo = Number.isFinite(min) ? Math.max(0, Math.round(min)) : 0
  const hi = Number.isFinite(max) ? Math.max(lo, Math.round(max)) : lo
  if (hi === 0) return 0
  return lo + Math.floor(Math.random() * (hi - lo + 1))
}

/** Fisher–Yates 洗牌（返回新数组，不改动入参）。 */
export function shuffleList<T>(list: readonly T[]): T[] {
  const copy = list.slice()
  for (let i = copy.length - 1; i > 0; i -= 1) {
    const j = Math.floor(Math.random() * (i + 1))
    const tmp = copy[i]
    copy[i] = copy[j]
    copy[j] = tmp
  }
  return copy
}
