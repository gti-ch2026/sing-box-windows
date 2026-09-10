import { loadSession, reportPikaTraffic, saveSession } from '@/services/pika-account-service'
import { useTrafficStore } from '@/stores/kernel/TrafficStore'

const MIN_BYTES = 1024
const INTERVAL_MS = 15_000

let timer: number | null = null
let lastUp = 0
let lastDown = 0
let flushing = false

const currentTotals = () => {
  const traffic = useTrafficStore().traffic
  return {
    up: Number(traffic.totalUp || 0),
    down: Number(traffic.totalDown || 0),
  }
}

const flush = async () => {
  if (flushing) return
  const session = loadSession()
  if (!session?.token) return

  const { up, down } = currentTotals()
  if (up < lastUp) lastUp = 0
  if (down < lastDown) lastDown = 0
  const upload = Math.max(0, up - lastUp)
  const download = Math.max(0, down - lastDown)
  if (upload + download < MIN_BYTES) return

  flushing = true
  lastUp = up
  lastDown = down
  try {
    const quota = await reportPikaTraffic(session.token, upload, download)
    saveSession({
      ...session,
      uploadBytes: quota.u,
      downloadBytes: quota.d,
      totalBytes: quota.transfer_enable || session.totalBytes,
    })
    const { usePikaAccountStore } = await import('@/stores/pika/AccountStore')
    usePikaAccountStore().applyQuota(quota)
  } catch (error) {
    lastUp = Math.max(0, lastUp - upload)
    lastDown = Math.max(0, lastDown - download)
    console.warn('上报流量失败:', error)
  } finally {
    flushing = false
  }
}

export const startPikaTrafficReporter = () => {
  stopPikaTrafficReporter()
  lastUp = currentTotals().up
  lastDown = currentTotals().down
  timer = window.setInterval(() => {
    void flush()
  }, INTERVAL_MS)
  window.addEventListener('beforeunload', onUnload)
}

export const stopPikaTrafficReporter = () => {
  if (timer != null) {
    clearInterval(timer)
    timer = null
  }
  window.removeEventListener('beforeunload', onUnload)
}

const onUnload = () => {
  void flush()
}
