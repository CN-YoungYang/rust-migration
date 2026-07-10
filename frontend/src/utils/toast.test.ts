import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { confirmDialogKeyAction } from './toast.ts'

describe('confirmDialogKeyAction', () => {
  it('使用 Escape 时取消确认框', () => {
    assert.equal(confirmDialogKeyAction('Escape'), 'cancel')
  })

  it('使用 Enter 时不绕过当前焦点自动确认', () => {
    assert.equal(confirmDialogKeyAction('Enter'), null)
  })
})
