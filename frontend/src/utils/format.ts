// 跨面板共享的日期/时间格式化助手

/** 短格式：MM-DD HH:mm；空值或非法时间返回 fallback */
export function formatDateTime(value: string | null | undefined, fallback = '无记录'): string {
  if (!value) return fallback
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return fallback
  return date.toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

/** 完整格式：toLocaleString('zh-CN')（含秒）；空值或非法时间返回 fallback */
export function formatDateTimeFull(value: string | null | undefined, fallback = '无效时间'): string {
  if (!value) return fallback
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return fallback
  return date.toLocaleString('zh-CN')
}

/** 浏览器本地时区的 `YYYY-MM-DD`，用于日期选择器与筛选输入 */
export function formatDateInput(date: Date): string {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}
