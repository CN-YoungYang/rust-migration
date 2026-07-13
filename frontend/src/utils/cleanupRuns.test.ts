import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { buildCleanupRequest, cleanupScopeLabel, cleanupTargetText } from './cleanupRuns.ts'

describe('buildCleanupRequest', () => {
  it('管理员筛选用户时把 userId 传入清理请求', () => {
    assert.deepEqual(buildCleanupRequest(0, true, 'user-2', true), {
      keepLatest: 0,
      userId: 'user-2',
      resetState: true,
    })
  })

  it('普通用户请求不携带可伪造的 userId', () => {
    assert.deepEqual(buildCleanupRequest(100, false, 'user-2', true), {
      keepLatest: 100,
    })
  })

  it('保留记录时不发送 resetState', () => {
    assert.deepEqual(buildCleanupRequest(50, true, '', true), {
      keepLatest: 50,
    })
  })
})

describe('cleanupScopeLabel', () => {
  it('明确区分全部用户、指定用户和当前用户范围', () => {
    assert.equal(cleanupScopeLabel(true, '', ''), '全部用户')
    assert.equal(cleanupScopeLabel(true, 'user-2', 'Alice'), '用户 Alice')
    assert.equal(cleanupScopeLabel(false, '', ''), '我的记录')
  })
})
describe('cleanupTargetText', () => {
  it('为确认文案生成自然的所有格范围', () => {
    assert.equal(cleanupTargetText(true, '', ''), '全部用户的')
    assert.equal(cleanupTargetText(true, 'user-2', 'Alice'), '用户 Alice 的')
    assert.equal(cleanupTargetText(false, '', ''), '我的')
  })
})