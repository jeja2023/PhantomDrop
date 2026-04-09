const apiBase = ((import.meta.env.VITE_API_BASE_URL as string | undefined) ?? '').replace(/\/$/, '')

export function buildApiUrl(path: string): string {
  if (!apiBase) {
    return path
  }

  return `${apiBase}${path.startsWith('/') ? path : `/${path}`}`
}

export async function parseApiError(response: Response): Promise<string> {
  const contentType = response.headers.get('content-type') ?? ''
  if (contentType.includes('application/json')) {
    const data = await response.json() as { message?: string; msg?: string }
    return data.message ?? data.msg ?? `请求失败: ${response.status}`
  }

  const text = await response.text()
  return text || `请求失败: ${response.status}`
}

export async function fetchJson<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(buildApiUrl(path), init)
  if (!response.ok) {
    throw new Error(await parseApiError(response))
  }

  const contentType = response.headers.get('content-type') ?? ''
  if (!contentType.includes('application/json')) {
    const text = await response.text()
    const preview = text.slice(0, 120).replace(/\s+/g, ' ').trim()
    throw new Error(`接口返回了非 JSON 内容，请检查 Vite 代理或 API 地址: ${preview}`)
  }

  return response.json() as Promise<T>
}

export async function postJson<TResponse, TBody>(path: string, body: TBody): Promise<TResponse> {
  return fetchJson<TResponse>(path, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(body),
  })
}

export async function deleteJson<TResponse>(path: string): Promise<TResponse> {
  return fetchJson<TResponse>(path, {
    method: 'DELETE',
  })
}

export function createApiEventSource(path: string): EventSource {
  return new EventSource(buildApiUrl(path))
}
