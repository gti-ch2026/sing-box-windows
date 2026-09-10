import { watch, type Ref } from 'vue'
import { Window } from '@tauri-apps/api/window'
import type { Router } from 'vue-router'
import mitt from '@/utils/mitt'
import { subscriptionService } from '@/services/subscription-service'
import { eventService } from '@/services/event-service'
import { APP_EVENTS } from '@/constants/events'
import type { UpdateAvailablePayload } from '@/types/events'

type MessageSetter = (message: ReturnType<typeof import('naive-ui').useMessage>) => void

export interface AppBootstrapDeps {
  router: Router
  localeRef: Ref<string>
  stores: {
    themeStore: {
      initializeStore: () => Promise<void>
      mode: string
      accentColor: string
      compactMode: boolean
    }
    appStore: {
      initializeStore: () => Promise<void>
      setMessageInstance: MessageSetter
      clearMessages: () => void
      setRunningState: (running: boolean) => void
      showWarningMessage?: (content: string) => void
      activeConfigPath: string | null
    }
    localeStore: {
      initializeStore: () => Promise<void>
      currentLocale: string
    }
    windowStore: {
      windowState: {
        isVisible: boolean
        lastVisiblePath?: string
      }
    }
    subStore: {
      initializeStore: () => Promise<void>
      getActiveSubscription: () => { configPath?: string; useOriginalConfig?: boolean } | null
    }
    pikaAccountStore?: {
      hydrate: () => void
      loggedIn: boolean
      refresh: () => Promise<void>
    }
    kernelStore: { initializeStore: () => Promise<void> }
    updateStore: {
      initializeStore: () => Promise<void>
      autoCheckUpdate: boolean
      applyUpdateInfo: (info: UpdateAvailablePayload) => void
    }
    trafficStore: { initializeStore: () => Promise<void> }
    connectionStore: { initializeStore: () => Promise<void> }
    logStore: { initializeStore: () => Promise<void>; cleanupListeners: () => void }
    trayStore: { initTray: () => Promise<boolean> }
  }
}

/**
 * 集中管理应用启动流程与清理，便于后续迁移调度逻辑到后端。
 */
export function useAppBootstrap(deps: AppBootstrapDeps) {
  const { router, localeRef, stores } = deps
  const {
    themeStore,
    appStore,
    localeStore,
    windowStore,
    subStore,
    kernelStore,
    updateStore,
    trafficStore,
    connectionStore,
    logStore,
    trayStore,
  } = stores

  const cleanupFns: Array<() => void> = []

  const checkInitialWindowState = async () => {
    const appWindow = Window.getCurrent()
    try {
      const [visible, minimized] = await Promise.all([
        appWindow.isVisible(),
        appWindow.isMinimized(),
      ])
      windowStore.windowState.isVisible = visible

      const { loadSession } = await import('@/services/pika-account-service')
      const loggedIn = Boolean(loadSession()?.token)
      const path = router.currentRoute.value.path
      if (path === '/blank') {
        await router.replace(loggedIn ? '/' : '/login')
      }
      if (!visible || minimized) {
        return
      }
    } catch (error) {
      console.error('检查初始窗口状态失败:', error)
    }
  }

  const setupMessageBus = () => {
    const handleMessageReady = (message: unknown) => {
      appStore.setMessageInstance(message as ReturnType<typeof import('naive-ui').useMessage>)
    }
    const handleClearMessages = () => {
      appStore.clearMessages()
    }
    mitt.on('message-instance-ready', handleMessageReady)
    mitt.on('clear-ui-messages', handleClearMessages)

    cleanupFns.push(() => mitt.off('message-instance-ready', handleMessageReady))
    cleanupFns.push(() => mitt.off('clear-ui-messages', handleClearMessages))
  }

  const setupLocaleWatcher = () => {
    const stopWatchingLocale = watch(
      () => localeStore.currentLocale,
      (newLocale) => {
        if (newLocale) {
          localeRef.value = newLocale
        }
      },
      { immediate: true },
    )
    cleanupFns.push(stopWatchingLocale)
  }

  const setupRouteWatcher = () => {
    const stopWatchingRoute = watch(
      () => router.currentRoute.value.path,
      (newPath) => {
        if (newPath === '/blank') {
          appStore.clearMessages()
        }
      },
    )
    cleanupFns.push(stopWatchingRoute)
  }

  const setupBackendEventBridge = () => {
    // 更新可用事件 -> 同步到更新 Store
    eventService
      .on(APP_EVENTS.updateAvailable, (payload) => {
        updateStore.applyUpdateInfo(payload)
        mitt.emit('update-available', payload)
      })
      .then((unlisten) => cleanupFns.push(unlisten))
      .catch((error) => {
        console.error('注册 update-available 事件失败:', error)
      })

    // 内核健康事件 - 仅记录日志，不向用户弹窗（在 Linux 上检测不准确）
    eventService
      .on(APP_EVENTS.kernelHealth, () => {
        // 仅记录日志，不向用户弹窗（在 Linux 上检测不准确）
      })
      .then((unlisten) => cleanupFns.push(unlisten))
      .catch((error) => {
        console.error('注册 kernel-health 事件失败:', error)
      })

    // 订阅刷新事件 -> 触发前端刷新或提示
    eventService
      .on(APP_EVENTS.subscriptionUpdated, () => {
        mitt.emit('subscription-updated')
      })
      .then((unlisten) => cleanupFns.push(unlisten))
      .catch((error) => {
        console.error('注册 subscription-updated 事件失败:', error)
      })
  }

  const initialize = async () => {
    await themeStore.initializeStore()
    await appStore.initializeStore()

    await subStore.initializeStore()
    try {
      const { usePikaAccountStore } = await import('@/stores/pika/AccountStore')
      const account = usePikaAccountStore()
      account.hydrate()
      if (account.loggedIn) {
        void account.refresh().catch((refreshError) => {
          console.warn('刷新 Pika 官方线路失败:', refreshError)
        })
      }
    } catch (error) {
      console.warn('刷新 Pika 官方线路失败:', error)
    }
    // 启动时优先使用 AppConfig.active_config_path（内核实际读取的“权威值”），
    // 再回退到订阅 Store 的高亮项，避免出现“高亮与内核配置不一致”。
    const activeSub = subStore.getActiveSubscription()
    const desiredConfigPath = appStore.activeConfigPath || activeSub?.configPath || null

    await localeStore.initializeStore()
    await updateStore.initializeStore()

    setupMessageBus()
    setupLocaleWatcher()
    setupRouteWatcher()

    await checkInitialWindowState()

    // 内核/托盘放到后面：卡住也不挡登录页和首页先画出来。
    void (async () => {
      try {
        if (desiredConfigPath) {
          await subscriptionService.setActiveConfig(desiredConfigPath, {
            useOriginalConfig: activeSub?.useOriginalConfig,
          })
        }
        await kernelStore.initializeStore()
        await logStore.initializeStore()
        cleanupFns.push(() => logStore.cleanupListeners())
        await Promise.allSettled([
          trafficStore.initializeStore(),
          connectionStore.initializeStore(),
        ])
        await trayStore.initTray()
        setupBackendEventBridge()
      } catch (error) {
        console.error('后台初始化失败:', error)
      }
    })()
  }

  const cleanup = () => {
    cleanupFns.forEach((fn) => {
      try {
        fn()
      } catch (error) {
        console.error('清理函数执行失败:', error)
      }
    })
    cleanupFns.length = 0
  }

  return { initialize, cleanup }
}
