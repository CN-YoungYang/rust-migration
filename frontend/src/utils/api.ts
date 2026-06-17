import { API_BASE } from '../config'

export function getToken(): string {
  return localStorage.getItem('token') || ''
}

export function authHeaders(): Record<string, string> {
  return { Authorization: `Bearer ${getToken()}` }
}

export async function request(url: string, options: RequestInit = {}): Promise<Response> {
  const response = await fetch(url, options)
  if (!response.ok) {
    // 401: token 过期或无效，清除本地状态并刷新页面
    if (response.status === 401) {
      localStorage.removeItem('token')
      window.location.reload()
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

export function apiUrl(path: string): string {
  return `${API_BASE}${path}`
}
