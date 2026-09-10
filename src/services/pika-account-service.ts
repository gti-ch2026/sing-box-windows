export interface PikaSession {
  token: string
  identifier: string
  planName: string
  expireAt: number
  subscribeUrl: string
  uploadBytes: number
  downloadBytes: number
  totalBytes: number
}

const SESSION_KEY = 'pika-session'
const DEVICE_KEY = 'pika-device-id'

const DEFAULT_CONTROL_PLANE = 'https://pika.ktyun.cc'

export function controlPlaneBase(): string {
  const fromEnv = (import.meta.env.VITE_PIKA_CONTROL_PLANE_URL as string | undefined)?.trim()
  if (fromEnv) return fromEnv.replace(/\/$/, '')
  return DEFAULT_CONTROL_PLANE
}

export function rewriteStaleSubscribeUrl(url: string, currentToken?: string): string {
  try {
    const parsed = new URL(url)
    const local = parsed.hostname === '127.0.0.1' || parsed.hostname === 'localhost'
    const ngrok = parsed.hostname.endsWith('.ngrok-free.app') || parsed.hostname.endsWith('.ngrok.io')
    if (!parsed.pathname.startsWith('/s/')) {
      return url
    }
    if (currentToken && parsed.pathname !== `/s/${currentToken}`) {
      return `${controlPlaneBase()}/s/${currentToken}`
    }
    if (ngrok) {
      return `${controlPlaneBase()}${parsed.pathname}`
    }
    if (local) {
      return url
    }
    return url
  } catch {
    return url
  }
}

function deviceId(): string {
  const existing = localStorage.getItem(DEVICE_KEY)?.trim()
  if (existing) return existing
  const id = crypto.randomUUID()
  localStorage.setItem(DEVICE_KEY, id)
  return id
}

function deviceName(): string {
  return navigator.platform || 'Pika Desktop'
}

function unwrapToken(raw: string): string {
  const value = raw.trim()
  return value.startsWith('Bearer ') ? value : value
}

// 统一 fetch：网络层异常翻译成用户能看懂的中文，别让 "Failed to fetch" 裸奔
async function fetchOrThrow(url: string, init: RequestInit): Promise<Response> {
  try {
    return await fetch(url, { ...init, signal: AbortSignal.timeout(10000) })
  } catch (err) {
    if (err instanceof DOMException && err.name === 'TimeoutError') {
      throw new Error(`连接超时，请确认控制面可访问：${controlPlaneBase()}`)
    }
    throw new Error(`控制面连不上，请确认服务已启动：${controlPlaneBase()}`)
  }
}

async function postJson(path: string, body: unknown, token?: string): Promise<Record<string, unknown>> {
  const headers: Record<string, string> = {
    Accept: 'application/json',
    'Content-Type': 'application/json',
  }
  if (token) {
    headers.Authorization = unwrapToken(token).startsWith('Bearer ')
      ? unwrapToken(token)
      : `Bearer ${unwrapToken(token)}`
  }
  const response = await fetchOrThrow(`${controlPlaneBase().replace(/\/$/, '')}${path}`, {
    method: 'POST',
    headers,
    body: JSON.stringify(body),
  })
  const json = (await response.json().catch(() => ({}))) as Record<string, unknown>
  if (!response.ok || json.status === 'fail') {
    throw new Error(String(json.message || '请求失败'))
  }
  return json
}

async function getJson(path: string, token: string): Promise<Record<string, unknown>> {
  const auth = unwrapToken(token).startsWith('Bearer ') ? unwrapToken(token) : `Bearer ${unwrapToken(token)}`
  const response = await fetchOrThrow(`${controlPlaneBase().replace(/\/$/, '')}${path}`, {
    headers: { Accept: 'application/json', Authorization: auth },
  })
  const json = (await response.json().catch(() => ({}))) as Record<string, unknown>
  if (!response.ok || json.status === 'fail') {
    throw new Error(String(json.message || '请求失败'))
  }
  return json
}

function parseSession(loginBody: Record<string, unknown>, subBody: Record<string, unknown>, fallbackId: string): PikaSession {
  const loginData = (loginBody.data || {}) as Record<string, unknown>
  const token = String(loginData.auth_data || loginData.token || '')
  if (!token) throw new Error('登录失败，请检查账号和密码')
  const data = (subBody.data || {}) as Record<string, unknown>
  const plan = (data.plan || {}) as Record<string, unknown>
  const subscribeUrl = String(data.subscribe_url || '')
  if (!subscribeUrl) throw new Error('还没有可用套餐，请先去后台购买')
  const expireAt = Number(data.expired_at || 0)
  if (expireAt > 0 && expireAt * 1000 < Date.now()) {
    throw new Error('套餐已到期，请先续费')
  }
  return {
    token,
    identifier: String(data.email || fallbackId),
    planName: String(plan.name || '已开通'),
    expireAt,
    subscribeUrl,
    uploadBytes: Number(data.u || 0),
    downloadBytes: Number(data.d || 0),
    totalBytes: Number(data.transfer_enable || 0),
  }
}

export function loadSession(): PikaSession | null {
  try {
    const raw = localStorage.getItem(SESSION_KEY)
    if (!raw) return null
    const parsed = JSON.parse(raw) as PikaSession
    if (!parsed?.token || !parsed.subscribeUrl) return null
    return parsed
  } catch {
    return null
  }
}

export function saveSession(session: PikaSession): void {
  localStorage.setItem(SESSION_KEY, JSON.stringify(session))
}

export function clearSession(): void {
  localStorage.removeItem(SESSION_KEY)
}

export async function loginPika(identifier: string, password: string): Promise<PikaSession> {
  const loginBody = await postJson('/api/v1/passport/auth/login', {
    email: identifier.trim(),
    password,
    device_id: deviceId(),
    device_name: deviceName(),
    platform: navigator.platform.toLowerCase().includes('mac')
      ? 'macos'
      : navigator.platform.toLowerCase().includes('win')
        ? 'windows'
        : 'linux',
  })
  const loginData = (loginBody.data || {}) as Record<string, unknown>
  const token = String(loginData.auth_data || loginData.token || '')
  const subBody = await getJson('/api/v1/user/getSubscribe', token)
  const session = parseSession(loginBody, subBody, identifier.trim())
  saveSession(session)
  return session
}

export async function refreshPikaSession(token: string, fallbackId = ''): Promise<PikaSession> {
  const subBody = await getJson('/api/v1/user/getSubscribe', token)
  const session = parseSession({ data: { auth_data: token } }, subBody, fallbackId)
  saveSession(session)
  return session
}

export async function reportPikaTraffic(
  token: string,
  upload: number,
  download: number,
): Promise<{ u: number; d: number; transfer_enable: number }> {
  const body = await postJson(
    '/api/v1/user/traffic/report',
    {
      upload: Math.max(0, Math.floor(upload)),
      download: Math.max(0, Math.floor(download)),
    },
    token,
  )
  const data = (body.data || body) as Record<string, unknown>
  return {
    u: Number(data.u || 0),
    d: Number(data.d || 0),
    transfer_enable: Number(data.transfer_enable || 0),
  }
}
