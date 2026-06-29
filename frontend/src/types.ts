export interface CurrentUser {
  id: string
  username: string
  role: string
  enabled?: boolean
}

export interface Account {
  id: string
  name: string
  siteType: string
  baseUrl?: string
  userId?: string | null
  ownerId?: string | null
  ownerName?: string | null
  authType?: string
  enabled?: boolean
  retryEnabled?: boolean
  lastBalance?: number | string | null
  lastStatus?: string | null
  lastMessage?: string | null
  customCheckinUrl?: string | null
  note?: string | null
  todayRuns?: number
  lastRunAt?: string | null
  lastBalanceAt?: string | null
  createdAt?: string
  updatedAt?: string
}

export interface AccountGroup {
  key: string
  label: string
  isSelf?: boolean
  items: Account[]
}
