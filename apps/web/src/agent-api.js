export function normalizeAgentUrl(value, fallback = globalThis.location?.origin) {
  const url = new URL(value.trim() || fallback)
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password || url.search || url.hash) throw new Error('Agent 地址必须是 HTTP(S) 地址，不能包含账号、密码、查询参数或片段')
  return url.href.replace(/\/+$/, '')
}

export function createSseDecoder(onEvent) {
  const decoder = new TextDecoder()
  let buffer = '', data = [], size = 0
  return chunk => {
    buffer += decoder.decode(chunk, { stream: true })
    let end
    while ((end = buffer.indexOf('\n')) >= 0) {
      const line = buffer.slice(0, end).replace(/\r$/, '')
      buffer = buffer.slice(end + 1)
      if (!line) {
        if (data.length) onEvent(JSON.parse(data.join('\n')))
        data = []; size = 0
      } else if (line.startsWith('data:')) { const value = line.slice(5).replace(/^ /, ''); data.push(value); size += value.length }
      if (size > 1024 * 1024) throw new Error('SSE 事件过大')
    }
    if (buffer.length > 1024 * 1024) throw new Error('SSE 数据过大')
  }
}

// Each connection owns its credential and outstanding requests; nothing is persisted here.
export function createAgentConnection(base, secret, fetcher = globalThis.fetch) {
  const controllers = new Set()
  let closed = false
  function controller() {
    if (closed) throw new DOMException('Agent connection closed', 'AbortError')
    const result = new AbortController(); controllers.add(result); return result
  }
  async function fetchApi(path, options, signal) {
    if (!path.startsWith('/v1/')) throw new Error('Unsupported Agent API path')
    const headers = new Headers(options.headers)
    if (secret) headers.set('authorization', `Bearer ${secret}`)
    // Same-origin requests reuse browser HTTP authentication, including SSE.
    // Never forward browser credentials to another Agent origin.
    const credentials = new URL(base).origin === globalThis.location?.origin ? 'same-origin' : 'omit'
    const response = await fetcher(`${base}${path}`, { ...options, headers, signal, credentials, redirect: 'error' })
    if (closed || signal.aborted) throw new DOMException('Agent connection closed', 'AbortError')
    if (!response.ok) {
      let detail=''
      try { detail=(await response.json()).error || '' } catch {}
      throw new Error(`Agent 请求失败：${response.status}${response.status === 401 ? '，请刷新页面并使用 crabot / 访问口令登录' : response.status === 403 ? '，当前来源或访问方式不被允许' : ''}${detail?' · '+String(detail).slice(0,1500):''}`)
    }
    return response
  }
  return {
    async request(path, options = {}) {
      const ctrl = controller()
      try {
        const response = await fetchApi(path, options, ctrl.signal)
        const value = await response.json()
        if (closed || ctrl.signal.aborted) throw new DOMException('Agent connection closed', 'AbortError')
        return value
      } finally { controllers.delete(ctrl) }
    },
    async stream(path, onEvent, onError, options = {}) {
      const ctrl = controller()
      try {
        const headers = new Headers(options.headers); headers.set('accept', 'text/event-stream')
        const response = await fetchApi(path, { ...options, headers }, ctrl.signal)
        if (!response.headers.get('content-type')?.includes('text/event-stream') || !response.body) throw new Error('Agent 未返回 SSE 流')
        const reader = response.body.getReader()
        const push = createSseDecoder(event => { if (!closed && !ctrl.signal.aborted) onEvent(event) })
        void (async () => {
          try {
            while (true) {
              const { done, value } = await reader.read()
              if (done) { if (!ctrl.signal.aborted && !closed) onError(new Error('事件流已断开，请重新打开会话')); break }
              push(value)
            }
          } catch (error) { if (!closed && !ctrl.signal.aborted) onError(error) }
          finally { controllers.delete(ctrl); reader.releaseLock() }
        })()
        return { close: () => ctrl.abort() }
      } catch (error) { ctrl.abort(); controllers.delete(ctrl); throw error }
    },
    close() { closed = true; secret = ''; for (const ctrl of controllers) ctrl.abort(); controllers.clear() },
  }
}
