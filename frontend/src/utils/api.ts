export const AUTH_EXPIRED_EVENT = 'ai-hub:auth-expired'

function readCookie(name: string): string {
  const prefix = `${name}=`
  return document.cookie
    .split(';')
    .map((part) => part.trim())
    .find((part) => part.startsWith(prefix))
    ?.slice(prefix.length) || ''
}

function isUnsafeMethod(method?: string): boolean {
  const normalized = (method || 'GET').toUpperCase()
  return ['POST', 'PUT', 'DELETE', 'PATCH'].includes(normalized)
}

async function errorMessage(response: Response): Promise<string> {
  const text = await response.text()
  try {
    const json = JSON.parse(text)
    // details 是后端填的具体原因（如 Validation 的"部分记录不存在或已被删除"），
    // error 是通用分类文案（"输入验证失败"），优先展示前者，用户才知道怎么处理。
    return json.details || json.error || json.message || `HTTP ${response.status}`
  } catch {
    return text || `HTTP ${response.status}`
  }
}

export async function request(url: string, options: RequestInit = {}): Promise<Response> {
  const headers = new Headers(options.headers)
  if (isUnsafeMethod(options.method) && !headers.has('X-CSRF-Token')) {
    const csrfToken = readCookie('csrf_token')
    if (csrfToken) headers.set('X-CSRF-Token', csrfToken)
  }

  const response = await fetch(url, {
    ...options,
    credentials: 'include',
    headers
  })
  if (!response.ok) {
    if (response.status === 401) {
      const isAuthProbe = url.includes('/auth/login') || url.includes('/auth/me')
      if (!isAuthProbe) {
        window.dispatchEvent(new CustomEvent(AUTH_EXPIRED_EVENT))
        throw new Error('登录已过期，请重新登录')
      }
      throw new Error(await errorMessage(response))
    }
    throw new Error(await errorMessage(response))
  }
  return response
}

export async function responseData<T>(response: Response): Promise<T> {
  const json = await response.json()
  if (json && typeof json === 'object' && 'data' in json) {
    return json.data as T
  }
  return json as T
}

export function apiUrl(path: string): string {
  return `/api${path}`
}
