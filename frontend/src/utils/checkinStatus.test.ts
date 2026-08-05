import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { checkinStatusTagType, checkinStatusText, triggerText } from './checkinStatus.ts'

describe('checkinStatusText', () => {
  it('映射已知状态为中文文案（含大小写归一）', () => {
    assert.equal(checkinStatusText('success'), '成功')
    assert.equal(checkinStatusText('failed'), '失败')
    assert.equal(checkinStatusText('skipped'), '跳过')
    assert.equal(checkinStatusText('already_checked'), '今日已签')
    assert.equal(checkinStatusText('pending'), '进行中')
    assert.equal(checkinStatusText('SUCCESS'), '成功')
  })

  it('空值表示未签到，未知状态原样返回', () => {
    assert.equal(checkinStatusText(null), '未签到')
    assert.equal(checkinStatusText(undefined), '未签到')
    assert.equal(checkinStatusText(''), '未签到')
    assert.equal(checkinStatusText('weird'), 'weird')
  })
})

describe('checkinStatusTagType', () => {
  it('按状态映射 Tag 颜色', () => {
    assert.equal(checkinStatusTagType('success'), 'success')
    assert.equal(checkinStatusTagType('already_checked'), 'success')
    assert.equal(checkinStatusTagType('failed'), 'error')
    assert.equal(checkinStatusTagType('pending'), 'warning')
    assert.equal(checkinStatusTagType('skipped'), 'default')
  })

  it('未知状态与空值回退 default', () => {
    assert.equal(checkinStatusTagType('weird'), 'default')
    assert.equal(checkinStatusTagType(null), 'default')
    assert.equal(checkinStatusTagType(undefined), 'default')
  })
})

describe('triggerText', () => {
  it('映射已知触发方式', () => {
    assert.equal(triggerText('manual'), '手动')
    assert.equal(triggerText('manual_batch'), '批量手动')
    assert.equal(triggerText('scheduled'), '定时')
  })

  it('未知触发方式原样返回', () => {
    assert.equal(triggerText('cron-job'), 'cron-job')
  })
})
