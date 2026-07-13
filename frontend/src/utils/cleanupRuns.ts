export interface CleanupRunsRequest {
  keepLatest: number
  userId?: string
  resetState?: boolean
}

export function buildCleanupRequest(
  keepLatest: number,
  isAdmin: boolean,
  filterUserId: string,
  resetState: boolean,
): CleanupRunsRequest {
  const request: CleanupRunsRequest = { keepLatest }
  if (isAdmin && filterUserId) request.userId = filterUserId
  if (keepLatest === 0 && resetState) request.resetState = true
  return request
}

export function cleanupScopeLabel(
  isAdmin: boolean,
  filterUserId: string,
  selectedUsername: string,
): string {
  if (!isAdmin) return '我的记录'
  if (!filterUserId) return '全部用户'
  return `用户 ${selectedUsername || filterUserId}`
}
export function cleanupTargetText(
  isAdmin: boolean,
  filterUserId: string,
  selectedUsername: string,
): string {
  if (!isAdmin) return '我的'
  if (!filterUserId) return '全部用户的'
  return `用户 ${selectedUsername || filterUserId} 的`
}