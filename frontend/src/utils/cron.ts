// 标准 5 段 cron 表达式校验（分 时 日 月 周），语义与后端 croner 对齐。
// 与后端 routes/settings.rs 的 Cron::from_str 校验保持同一口径：croner 接受即通过。
// croner 扩展语法都支持：? 通配、L（最后）/ W（最近工作日）/ #（第 N 个星期）修饰符、
// 字母月份/星期名、星期 7=周日、字母星期环回（如 SAT-SUN，内部转 6-7 升序）、逗号列表空 token 容忍。

type RangeDef = {
  min: number
  max: number
  names: [string, number][]
  // 字母星期名映射表（仅日段 L、星期段 L/# 用到）
  weekdayNames?: [string, number][]
}

const MONTH_NAMES: [string, number][] = [
  ['jan', 1], ['feb', 2], ['mar', 3], ['apr', 4], ['may', 5], ['jun', 6],
  ['jul', 7], ['aug', 8], ['sep', 9], ['oct', 10], ['nov', 11], ['dec', 12],
]

// croner 的星期映射：SUN=0..SAT=6，且 7 也归一为 0。字母名环回（SAT-SUN）会先把
// 右端字母名映射到 7 再判升序，故此表里 SAT=6、SUN=7 仅为环回判断用，匹配仍按 0=周日。
const WEEKDAY_NAMES_FOR_WRAP: [string, number][] = [
  ['sun', 7], ['mon', 1], ['tue', 2], ['wed', 3], ['thu', 4], ['fri', 5], ['sat', 6],
]
const WEEKDAY_NAMES: [string, number][] = [
  ['sun', 0], ['mon', 1], ['tue', 2], ['wed', 3], ['thu', 4], ['fri', 5], ['sat', 6],
]

const FIELD_RANGES: RangeDef[] = [
  { min: 0, max: 59, names: [] },
  { min: 0, max: 23, names: [] },
  { min: 1, max: 31, names: [] },
  { min: 1, max: 12, names: MONTH_NAMES },
  { min: 0, max: 7, names: WEEKDAY_NAMES, weekdayNames: WEEKDAY_NAMES_FOR_WRAP },
]

const FIELD_LABELS = ['分钟', '小时', '日', '月', '星期']

const DOM_INDEX = 2
const DOW_INDEX = 4

// 把一个原始 token 解析为数值；字母名走 names 映射，数字走 Number 解析；无效返回 null。
function resolveValue(token: string, names: [string, number][]): number | null {
  const lower = token.toLowerCase()
  for (const [name, value] of names) {
    if (name === lower) return value
  }
  const n = Number(token)
  return Number.isInteger(n) ? n : null
}

// 判断 token 是否为字母星期名（SUN..SAT，大小写无关）。
function isAlphaWeekday(token: string, names: [string, number][] | undefined): boolean {
  if (!names) return false
  const lower = token.toLowerCase()
  return names.some(([name]) => name === lower)
}

// 检查单个字段（不含逗号）。fieldIndex 用于判断是否允许 ?/L/W/# 等修饰符。
// namesForWrap：星期字段的字母环回判断用另一张表（SUN=7，使 SAT-SUN 升序合法）。
function fieldOk(
  field: string,
  range: RangeDef,
  fieldIndex: number,
): boolean {
  const allowQuestion = fieldIndex === DOM_INDEX || fieldIndex === DOW_INDEX
  const allowDomModifiers = fieldIndex === DOM_INDEX || fieldIndex === DOW_INDEX // L / W / #

  // croner 对逗号列表里的空 token 容忍（跳过），与 `1,,2` 等价于 `1,2`。
  const tokens = field.split(',')
  const soleToken = tokens.length === 1
  for (const token of tokens) {
    if (token === '') continue
    const slashParts = token.split('/')
    if (slashParts.length > 2) return false
    const [baseRaw, stepStr] = slashParts
    // 剥离末尾修饰符（L / W / #N），供数值/区间段解析。
    const { core: base, modifier } = stripModifier(baseRaw, fieldIndex)
    if (modifier !== null && !allowDomModifiers) return false

    let lo: number
    let hi: number
    // '?' 仅作为整段通配合法（croner 对列表内 '?' 报 Invalid number）。
    if (base === '*' || (allowQuestion && base === '?' && soleToken)) {
      lo = range.min
      hi = range.max
    } else {
      const rangeParts = base.split('-')
      if (rangeParts.length > 2) return false
      // croner 对字母星期名做特殊归一：SAT-SUN、SUN-SAT 都接受（内部 replace_alpha_weekdays
      // 先把 `-SUN`→`-7` 再 `SUN`→`0`，任意两端字母名组合最终都成升序）；对纯数值则严格升序
      // （6-0 / 7-0 报 Range out of bounds）。因此星期字段分两种口径：
      //   - 两端都是字母名：用 wrap 表（SUN=7）解析，并跳过降序校验（交给 croner 归一）；
      //   - 至少一端是数值：用原表（SUN=0），按数值升序严格校验。
      const useWrap = fieldIndex === DOW_INDEX && isAlphaWeekday(rangeParts[0], range.weekdayNames) && (rangeParts.length === 1 || isAlphaWeekday(rangeParts[1], range.weekdayNames))
      const namesForField = useWrap ? (range.weekdayNames ?? range.names) : range.names
      const loVal = resolveValue(rangeParts[0], namesForField)
      const hiVal =
        rangeParts.length === 2
          ? resolveValue(rangeParts[1], namesForField)
          : loVal
      if (loVal === null || hiVal === null) return false
      lo = loVal
      hi = hiVal
      if (lo < range.min || hi > range.max) return false
      // 字母星期名两端组合 croner 会归一成升序，跳过降序校验；纯数值严格升序。
      if (!useWrap && lo > hi) return false
    }

    if (stepStr !== undefined) {
      const step = Number(stepStr)
      if (!Number.isInteger(step) || step <= 0) return false
    }
  }
  return true
}

// 剥离并校验 croner 的 L/W/# 修饰符。返回 { core, modifier }；modifier 形如 'L' | 'W' | '#' | null。
// '15W'、'L'、'5L'、'5#3'、'FRI#L' 等。core 为去掉修饰后的主体（用于后续数值/区间解析）。
function stripModifier(raw: string, fieldIndex: number): { core: string; modifier: string | null } {
  // '#' 第 N 个星期：形如 5#3 或 FRI#L（croner 支持 #L 表示最后一个对应星期）
  const hashIdx = raw.indexOf('#')
  if (hashIdx !== -1) {
    if (fieldIndex !== DOW_INDEX) return { core: raw, modifier: null }
    const left = raw.slice(0, hashIdx)
    const right = raw.slice(hashIdx + 1)
    // 右侧可为数字或 L
    if (right === 'L' || (Number.isInteger(Number(right)) && Number(right) >= 1 && Number(right) <= 5)) {
      return { core: left, modifier: '#' }
    }
    return { core: raw, modifier: null }
  }
  // 'W' 最近工作日：仅日段，形如 15W
  const wIdx = raw.lastIndexOf('W')
  if (wIdx === raw.length - 1 && wIdx !== -1) {
    if (fieldIndex !== DOM_INDEX) return { core: raw, modifier: null }
    const core = raw.slice(0, wIdx)
    if (!core) return { core: raw, modifier: null }
    return { core, modifier: 'W' }
  }
  // 'L' 最后一个：日段 L、星期段 5L / FRIL
  const lIdx = raw.lastIndexOf('L')
  if (lIdx === raw.length - 1 && lIdx !== -1) {
    if (fieldIndex !== DOM_INDEX && fieldIndex !== DOW_INDEX) return { core: raw, modifier: null }
    const core = raw.slice(0, lIdx)
    // 纯 'L'（日段）或 '5L'/'FRIL'（星期段）均合法
    return { core: core === '' ? '*' : core, modifier: 'L' }
  }
  return { core: raw, modifier: null }
}

/** 校验标准 5 段 cron 表达式；合法返回 null，否则返回中文错误信息。 */
export function validateCronExpr(expr: string): string | null {
  const trimmed = expr.trim()
  if (!trimmed) return 'cron 表达式不能为空'
  const fields = trimmed.split(/\s+/)
  if (fields.length !== 5) {
    return `cron 表达式应为 5 段（分 时 日 月 周），当前 ${fields.length} 段`
  }
  for (let i = 0; i < fields.length; i++) {
    if (!fieldOk(fields[i], FIELD_RANGES[i], i)) {
      return `第 ${i + 1} 段（${FIELD_LABELS[i]}）格式无效`
    }
  }
  return null
}
