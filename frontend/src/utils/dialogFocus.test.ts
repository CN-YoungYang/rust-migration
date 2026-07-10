import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { wrappedDialogFocusIndex } from './dialogFocus.ts'

describe('wrappedDialogFocusIndex', () => {
  it('在首个元素按 Shift+Tab 时循环到末尾', () => {
    assert.equal(wrappedDialogFocusIndex(0, 2, true), 1)
  })

  it('在末尾元素按 Tab 时循环到开头', () => {
    assert.equal(wrappedDialogFocusIndex(1, 2, false), 0)
  })

  it('未到边界时交给浏览器处理', () => {
    assert.equal(wrappedDialogFocusIndex(1, 3, true), null)
  })
})
