import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { accountFormFields } from './accountForm.ts'

describe('accountFormFields', () => {
  it('new-api access_token 只显示用户 ID、认证方式和 Token', () => {
    assert.deepEqual(accountFormFields('new-api', 'access_token'), {
      userId: true,
      authType: true,
      accessToken: true,
      cookie: false,
      customCheckinUrl: false,
    })
  })

  it('new-api cookie 认证隐藏 Token 并显示 Cookie', () => {
    assert.deepEqual(accountFormFields('new-api', 'cookie'), {
      userId: true,
      authType: true,
      accessToken: false,
      cookie: true,
      customCheckinUrl: false,
    })
  })

  it('anyrouter 只显示 userId、Cookie 和自定义签到 URL', () => {
    assert.deepEqual(accountFormFields('anyrouter', 'cookie'), {
      userId: true,
      authType: false,
      accessToken: false,
      cookie: true,
      customCheckinUrl: true,
    })
  })

  it('x666 只显示 Cookie 和自定义签到 URL', () => {
    assert.deepEqual(accountFormFields('x666', 'cookie'), {
      userId: false,
      authType: false,
      accessToken: false,
      cookie: true,
      customCheckinUrl: true,
    })
  })
})
