import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import {
  batchSkipReason,
  isSameLocalDay,
  randomDelaySecs,
  shuffleList,
} from './batchCheckin.ts'

function todayIso(hourOffset = 0): string {
  const now = new Date()
  now.setHours(now.getHours() + hourOffset)
  return now.toISOString()
}

describe('isSameLocalDay', () => {
  it('空值与非法时间串返回 false', () => {
    assert.equal(isSameLocalDay(null), false)
    assert.equal(isSameLocalDay(undefined), false)
    assert.equal(isSameLocalDay(''), false)
    assert.equal(isSameLocalDay('not-a-date'), false)
  })

  it('今天的时间串返回 true，昨天返回 false', () => {
    assert.equal(isSameLocalDay(todayIso(0)), true)
    assert.equal(isSameLocalDay(todayIso(-24)), false)
    assert.equal(isSameLocalDay(todayIso(-48)), false)
  })
})

describe('batchSkipReason', () => {
  const settings = { retryEnabled: true, maxAttemptsPerDay: 3 }

  it('已禁用账户直接跳过', () => {
    assert.equal(batchSkipReason({ enabled: false }, settings), '账户已禁用')
  })

  it('今日已签（success / already_checked）跳过', () => {
    assert.equal(
      batchSkipReason({ lastRunAt: todayIso(0), lastStatus: 'success' }, settings),
      '今日已签到',
    )
    assert.equal(
      batchSkipReason({ lastRunAt: todayIso(0), lastStatus: 'already_checked' }, settings),
      '今日已签到',
    )
  })

  it('昨日成功不跳过（只关心今天）', () => {
    assert.equal(
      batchSkipReason({ lastRunAt: todayIso(-24), lastStatus: 'success' }, settings),
      null,
    )
  })

  it('今日尝试且账户关闭重试 -> 重试已关闭', () => {
    assert.equal(
      batchSkipReason(
        { lastRunAt: todayIso(0), lastStatus: 'failed', retryEnabled: false },
        settings,
      ),
      '重试已关闭',
    )
  })

  it('今日尝试且全局关闭重试 -> 重试已关闭', () => {
    assert.equal(
      batchSkipReason(
        { lastRunAt: todayIso(0), lastStatus: 'failed', retryEnabled: true },
        { ...settings, retryEnabled: false },
      ),
      '重试已关闭',
    )
  })

  it('今日尝试但重试开启 -> 需要执行', () => {
    assert.equal(
      batchSkipReason(
        { lastRunAt: todayIso(0), lastStatus: 'failed', retryEnabled: true },
        settings,
      ),
      null,
    )
  })

  it('已达每日尝试上限 -> 跳过', () => {
    assert.equal(batchSkipReason({ todayRuns: 3 }, settings), '已达今日尝试上限（3）')
    assert.equal(batchSkipReason({ todayRuns: 5 }, settings), '已达今日尝试上限（3）')
    assert.equal(batchSkipReason({ todayRuns: 2 }, settings), null)
  })

  it('无设置（普通用户）时不套用每日上限与全局重试', () => {
    assert.equal(batchSkipReason({ todayRuns: 99 }, null), null)
    assert.equal(
      batchSkipReason({ lastRunAt: todayIso(0), lastStatus: 'failed', retryEnabled: true }, null),
      null,
    )
    assert.equal(
      batchSkipReason({ lastRunAt: todayIso(0), lastStatus: 'failed', retryEnabled: false }, null),
      '重试已关闭',
    )
  })

  it('账户不存在时按可执行处理', () => {
    assert.equal(batchSkipReason(undefined, settings), null)
  })
})

describe('randomDelaySecs', () => {
  it('0~0 表示不延迟', () => {
    assert.equal(randomDelaySecs(0, 0), 0)
  })

  it('min === max 时为固定值', () => {
    assert.equal(randomDelaySecs(3, 3), 3)
  })

  it('随机值落在 [min, max] 闭区间内', () => {
    for (let i = 0; i < 500; i += 1) {
      const value = randomDelaySecs(2, 5)
      assert.ok(value >= 2 && value <= 5, `实际值 ${value}`)
    }
  })

  it('非数值入参回退为 0', () => {
    assert.equal(randomDelaySecs(Number.NaN, Number.NaN), 0)
  })
})

describe('shuffleList', () => {
  it('返回相同元素的重排', () => {
    const list = [1, 2, 3, 4, 5]
    const shuffled = shuffleList(list)
    assert.equal(shuffled.length, list.length)
    assert.deepEqual([...shuffled].sort((a, b) => a - b), [...list].sort((a, b) => a - b))
  })

  it('不改动入参数组', () => {
    const list = [1, 2, 3]
    shuffleList(list)
    assert.deepEqual(list, [1, 2, 3])
  })

  it('空数组返回空数组', () => {
    assert.deepEqual(shuffleList([]), [])
  })
})
