import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { businessDayBoundary, formatDateInput, formatDateTime, formatDateTimeFull } from './format.ts'

describe('formatDateTime', () => {
  it('输出 zh-CN 短格式（MM/DD HH:mm）', () => {
    const input = '2026-08-05T10:20:00'
    const expected = new Date(input).toLocaleString('zh-CN', {
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    })
    assert.equal(formatDateTime(input), expected)
  })

  it('空值与非法时间返回默认 fallback「无记录」', () => {
    assert.equal(formatDateTime(null), '无记录')
    assert.equal(formatDateTime(undefined), '无记录')
    assert.equal(formatDateTime(''), '无记录')
    assert.equal(formatDateTime('not-a-date'), '无记录')
  })

  it('可自定义 fallback', () => {
    assert.equal(formatDateTime('', '—'), '—')
  })
})

describe('formatDateTimeFull', () => {
  it('输出与 toLocaleString 一致的完整本地时间', () => {
    const input = '2026-08-05T10:20:30'
    assert.equal(formatDateTimeFull(input), new Date(input).toLocaleString('zh-CN'))
  })

  it('非法时间返回默认 fallback「无效时间」', () => {
    assert.equal(formatDateTimeFull('nope'), '无效时间')
    assert.equal(formatDateTimeFull(null), '无效时间')
  })
})

describe('formatDateInput', () => {
  it('输出本地时区 YYYY-MM-DD 并补零', () => {
    assert.equal(formatDateInput(new Date(2026, 0, 5)), '2026-01-05')
    assert.equal(formatDateInput(new Date(2026, 11, 31)), '2026-12-31')
  })
})

describe('businessDayBoundary', () => {
  it('按 Asia/Shanghai 业务时区生成 UTC 日界，而不是浏览器本地日界', () => {
    assert.equal(businessDayBoundary('2026-09-18', false), '2026-09-17T16:00:00.000Z')
    assert.equal(businessDayBoundary('2026-09-18', true), '2026-09-18T15:59:59.999Z')
  })
})
