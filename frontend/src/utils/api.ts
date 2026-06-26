import { API_BASE } from '../config'

export function getToken(): string {
  return ''
}

export function authHeaders(): Record<string, string> {
  return {}
}

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
    // 401: session 过期或无效，由调用方切回登录态
    if (response.status === 401) {
      throw new Error('登录已过期，请重新登录')
    }
    const text = await response.text()
    try {
      const json = JSON.parse(text)
      throw new Error(json.error || json.message || json.details || `HTTP ${response.status}`)
    } catch (e) {
      if (e instanceof SyntaxError) {
        throw new Error(text || `HTTP ${response.status}`)
      }
      throw e
    }
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
  return `${API_BASE}${path}`
}
