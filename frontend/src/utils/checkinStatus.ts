// 签到/账户状态与触发方式的展示映射（文案 + Tag 颜色）
// 后端状态值统一为小写（success / failed / skipped / already_checked / pending）

const STATUS_TEXT: Record<string, string> = {
  success: '成功',
  failed: '失败',
  skipped: '跳过',
  already_checked: '今日已签',
  pending: '进行中',
}

/** 状态文案；空值表示“未签到”，未知状态原样返回 */
export function checkinStatusText(status: string | null | undefined): string {
  if (!status) return '未签到'
  return STATUS_TEXT[status.toLowerCase()] || status
}

/** 状态 Tag 颜色：成功/今日已签=success，失败=error，进行中=warning，其余=default */
export function checkinStatusTagType(status: string | null | undefined): 'default' | 'success' | 'error' | 'warning' {
  const normalized = status?.toLowerCase()
  if (normalized === 'success' || normalized === 'already_checked') return 'success'
  if (normalized === 'failed') return 'error'
  if (normalized === 'pending') return 'warning'
  return 'default'
}

const TRIGGER_TEXT: Record<string, string> = {
  manual: '手动',
  manual_batch: '批量手动',
  scheduled: '定时',
}

/** 触发方式文案；未知值原样返回 */
export function triggerText(trigger: string): string {
  return TRIGGER_TEXT[trigger] || trigger
}
