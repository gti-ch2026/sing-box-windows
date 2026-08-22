import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import {
  clearSession,
  loadSession,
  loginPika,
  refreshPikaSession,
  type PikaSession,
} from '@/services/pika-account-service'
import { subscriptionService } from '@/services/subscription-service'
import { generateConfigFileName } from '@/views/sub/subscription-utils'
import { useSubStore } from '@/stores/subscription/SubStore'
import { useAppStore } from '@/stores/app/AppStore'
import { useKernelStore } from '@/stores/kernel/KernelStore'

const OFFICIAL_NAME = 'Pika 官方线路'

export const usePikaAccountStore = defineStore('pika-account', () => {
  const session = ref<PikaSession | null>(loadSession())
  const loading = ref(false)
  const error = ref('')

  const loggedIn = computed(() => Boolean(session.value?.token && session.value.subscribeUrl))
  const planHint = computed(() => {
    if (!session.value) return ''
    if (session.value.expireAt > 0) {
      return `${session.value.planName} · ${new Date(session.value.expireAt * 1000).toLocaleDateString()}`
    }
    return session.value.planName
  })

  const usedBytes = computed(() => {
    if (!session.value) return 0
    return session.value.uploadBytes + session.value.downloadBytes
  })

  const remainingBytes = computed(() => {
    if (!session.value) return 0
    return Math.max(session.value.totalBytes - usedBytes.value, 0)
  })

  const hydrate = () => {
    session.value = loadSession()
  }

  const applyOfficialSubscription = async (next: PikaSession) => {
    const subStore = useSubStore()
    const appStore = useAppStore()
    const kernelStore = useKernelStore()
    const result = await subscriptionService.downloadSubscription(next.subscribeUrl, false, {
      fileName: generateConfigFileName('pika-official'),
      applyRuntime: false,
    })
    const existing = subStore.list.findIndex((item) => item.name === OFFICIAL_NAME)
    const item = {
      name: OFFICIAL_NAME,
      url: next.subscribeUrl,
      isLoading: false,
      lastUpdate: Date.now(),
      isManual: false,
      useOriginalConfig: false,
      configPath: result.configPath,
      backupPath: `${result.configPath}.bak`,
      autoUpdateIntervalMinutes: 360,
      subscriptionUpload: result.subscriptionUpload,
      subscriptionDownload: result.subscriptionDownload,
      subscriptionTotal: result.subscriptionTotal,
      subscriptionExpire: result.subscriptionExpire ?? next.expireAt,
    }
    if (existing >= 0) {
      subStore.list[existing] = { ...subStore.list[existing], ...item }
      await subStore.setActiveIndex(existing)
    } else {
      subStore.list.unshift(item)
      await subStore.setActiveIndex(0)
    }
    await subscriptionService.setActiveConfig(result.configPath, { useOriginalConfig: false })
    await appStore.setActiveConfigPath(result.configPath)
    await appStore.toggleSystemProxy(true)
    await kernelStore.applyProxySettings()
    await kernelStore.restartKernel()
  }

  const login = async (identifier: string, password: string) => {
    loading.value = true
    error.value = ''
    try {
      const next = await loginPika(identifier, password)
      session.value = next
      try {
        await applyOfficialSubscription(next)
      } catch (err) {
        error.value = err instanceof Error ? err.message : '登录成功，但线路还没连上'
      }
    } catch (err) {
      error.value = err instanceof Error ? err.message : '登录失败'
      throw err
    } finally {
      loading.value = false
    }
  }

  const refresh = async () => {
    if (!session.value?.token) return
    const next = await refreshPikaSession(session.value.token, session.value.identifier)
    session.value = next
    await applyOfficialSubscription(next)
  }

  const logout = async () => {
    const kernelStore = useKernelStore()
    const appStore = useAppStore()
    try {
      await kernelStore.stopKernel()
      await appStore.toggleSystemProxy(false)
      await kernelStore.applyProxySettings()
    } catch {
      /* 退出时内核可能本来就没在跑 */
    }
    clearSession()
    session.value = null
  }

  return {
    session,
    loading,
    error,
    loggedIn,
    planHint,
    usedBytes,
    remainingBytes,
    hydrate,
    login,
    refresh,
    logout,
  }
})
