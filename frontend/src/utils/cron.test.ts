import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { validateCronExpr } from './cron.ts'

describe('validateCronExpr', () => {
  it('接受合法表达式', () => {
    assert.equal(validateCronExpr('* * * * *'), null)
    assert.equal(validateCronExpr('0 3 * * *'), null)
    assert.equal(validateCronExpr('*/5 2-5 * * *'), null)
    assert.equal(validateCronExpr('*/30 * * * *'), null)
    assert.equal(validateCronExpr('0 8,20 * * *'), null)
    assert.equal(validateCronExpr('30 20 * * 1-5'), null)
    assert.equal(validateCronExpr('0 2 * * 0'), null)
    assert.equal(validateCronExpr('15,45 6-9 * * 7'), null)
  })

  it('接受月份/星期英文名（与后端 croner 对齐）', () => {
    assert.equal(validateCronExpr('0 0 * JAN *'), null)
    assert.equal(validateCronExpr('0 9 * * MON-FRI'), null)
    assert.equal(validateCronExpr('0 0 1 1 *'), null)
  })

  it('接受 croner 扩展修饰符 L/W/#（与后端 croner 对齐）', () => {
    assert.equal(validateCronExpr('0 0 L * *'), null) // 月最后一天
    assert.equal(validateCronExpr('0 0 15W * *'), null) // 最近工作日
    assert.equal(validateCronExpr('0 0 * * 5#3'), null) // 第 3 个周五
    assert.equal(validateCronExpr('0 0 * * 5L'), null) // 最后一个周五
    assert.equal(validateCronExpr('0 0 * * FRI#L'), null) // 最后一个周五（字母名 + #L）
  })

  it("'?' 仅限日/星期两段（与后端 croner 对齐）", () => {
    assert.equal(validateCronExpr('0 0 ? * *'), null) // 日段 '?'
    assert.equal(validateCronExpr('0 0 * * ?'), null) // 星期段 '?'
    assert.match(validateCronExpr('? * * * *') ?? '', /无效/) // 分钟段 '?' 应拒绝
    assert.match(validateCronExpr('0 ? * * *') ?? '', /无效/) // 小时段 '?' 应拒绝
  })

  it('接受字母星期环回（与后端 croner 对齐：内部转升序）', () => {
    assert.equal(validateCronExpr('0 0 * * SAT-SUN'), null) // croner 内部 -SUN→-7 变 6-7 升序
    assert.equal(validateCronExpr('0 0 * * SUN-SAT'), null) // 0-6 升序
  })

  it('拒绝数值星期环回区间（croner 对 start>end 数值报错）', () => {
    assert.match(validateCronExpr('0 0 * * 5-1') ?? '', /无效/) // 5→1 数值降序
    assert.match(validateCronExpr('0 0 * * 6-0') ?? '', /无效/) // 6→0 数值降序
  })

  it('前端对字母星期降序组合放宽（交给后端 croner 权威判定）', () => {
    // croner 拒绝 FRI-MON（5-1），前端不拦，保存时后端会返回 400；
    // 这样避免前端误杀合法表达式（如 SAT-SUN），把语义判断交给后端权威校验。
    assert.equal(validateCronExpr('0 0 * * FRI-MON'), null)
  })

  it('接受逗号列表里的空 token 与单值带步长（croner 容忍）', () => {
    assert.equal(validateCronExpr('1,,2 * * * *'), null) // 空元素被跳过
    assert.equal(validateCronExpr('1 0 * * 1/10'), null) // 单值带步长
  })

  it('拒绝空串与段数错误', () => {
    assert.match(validateCronExpr('') ?? '', /不能为空/)
    assert.match(validateCronExpr('   ') ?? '', /不能为空/)
    assert.match(validateCronExpr('* * *') ?? '', /5 段/)
    assert.match(validateCronExpr('* * * * * *') ?? '', /5 段/)
  })

  it('拒绝越界数值', () => {
    assert.match(validateCronExpr('60 * * * *') ?? '', /分钟/)
    assert.match(validateCronExpr('0 24 * * *') ?? '', /小时/)
    assert.match(validateCronExpr('0 0 32 * *') ?? '', /日/)
    assert.match(validateCronExpr('0 0 * 13 *') ?? '', /月/)
    assert.match(validateCronExpr('0 0 * * 8') ?? '', /星期/)
  })

  it('拒绝非法语法', () => {
    assert.match(validateCronExpr('*//5 * * * *') ?? '', /无效/)
    assert.match(validateCronExpr('a * * * *') ?? '', /无效/)
    assert.match(validateCronExpr('*/0 * * * *') ?? '', /无效/)
    assert.match(validateCronExpr('5-10-3 * * * *') ?? '', /无效/)
    // '?' 出现在日段逗号列表里也应被拒（croner 对 list 内 '?' 报错）
    assert.match(validateCronExpr('0 0 1,?,15 * *') ?? '', /无效/)
  })
})
