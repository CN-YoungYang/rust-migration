import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import {
  batchStatusText,
  buildWorkbenchQuery,
  isTerminalBatchStatus,
  mergeTrackedBatchIds,
  retryableFailedAccountIds,
  selectableAccountIds,
} from './workbench.ts'

describe('签到工作台交互模型', () => {
  it('构建分页和筛选参数，并限制普通用户不能带所属用户筛选', () => {
    const query = buildWorkbenchQuery(
      {
        keyword: '  demo  ',
        siteType: 'new-api',
        userId: 'other-user',
        enabled: 'true',
        status: 'failed',
        todayResult: 'failed',
        balanceWarning: 'true',
      },
      { isAdmin: false, page: 2, limit: 50 },
    )
    const params = new URLSearchParams(query)
    assert.equal(params.get('offset'), '50')
    assert.equal(params.get('keyword'), 'demo')
    assert.equal(params.get('userId'), null)
    assert.equal(params.get('todayResult'), 'failed')
  })

  it('默认只选中服务端标记为可执行的账号', () => {
    assert.deepEqual(
      selectableAccountIds([
        { id: 'a1', enabled: true, selectable: true },
        { id: 'a2', enabled: true, selectable: false },
        { id: 'a3', enabled: false, selectable: true },
      ]),
      ['a1'],
    )
  })

  it('失败重试只纳入失败且未关闭重试的账号', () => {
    assert.deepEqual(
      retryableFailedAccountIds(
        [
          { accountId: 'a1', status: 'failed' },
          { accountId: 'a2', status: 'failed' },
          { accountId: 'a3', status: 'success' },
          { accountId: 'a1', status: 'failed' },
        ],
        [
          { id: 'a1', enabled: true, retryEnabled: true },
          { id: 'a2', enabled: true, retryEnabled: false },
        ],
      ),
      ['a1'],
    )
  })

  it('只把终态批次从轮询集合中移除', () => {
    assert.equal(isTerminalBatchStatus('completed'), true)
    assert.equal(isTerminalBatchStatus('partial_failed'), true)
    assert.equal(isTerminalBatchStatus('running'), false)
    assert.equal(isTerminalBatchStatus(null), false)
    assert.equal(batchStatusText('partial_failed'), '部分失败')
  })

  it('合并批次编号时去重，并把新编号放在前面', () => {
    assert.deepEqual(
      mergeTrackedBatchIds(['old', 'same'], ['new', 'same', ''], 3),
      ['new', 'same', 'old'],
    )
  })
})
