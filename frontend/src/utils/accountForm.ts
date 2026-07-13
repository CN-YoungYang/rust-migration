export interface AccountFormFields {
  userId: boolean
  authType: boolean
  accessToken: boolean
  cookie: boolean
  customCheckinUrl: boolean
}

export function accountFormFields(siteType: string, authType: string): AccountFormFields {
  if (siteType === 'anyrouter') {
    return {
      userId: true,
      authType: false,
      accessToken: false,
      cookie: true,
      customCheckinUrl: true,
    }
  }

  if (siteType === 'x666') {
    return {
      userId: false,
      authType: false,
      accessToken: false,
      cookie: true,
      customCheckinUrl: true,
    }
  }

  return {
    userId: true,
    authType: true,
    accessToken: authType === 'access_token',
    cookie: authType === 'cookie',
    customCheckinUrl: false,
  }
}
