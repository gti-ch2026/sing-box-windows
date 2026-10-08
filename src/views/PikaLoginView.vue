<template>
  <div class="login-shell">
    <form class="login-card" autocomplete="on" @submit.prevent="onSubmit">
      <h1>{{ t('login.brand') }}</h1>
      <p class="lead">{{ t('login.lead') }}</p>

      <label class="field" for="pika-login-username">
        <span>{{ t('login.account') }}</span>
        <input
          id="pika-login-username"
          v-model="email"
          name="username"
          type="text"
          inputmode="email"
          autocomplete="username"
          autocapitalize="off"
          autocorrect="off"
          spellcheck="false"
          :placeholder="t('login.accountPlaceholder')"
        />
      </label>

      <label class="field" for="pika-login-password">
        <span>{{ t('login.password') }}</span>
        <input
          id="pika-login-password"
          v-model="password"
          name="password"
          type="password"
          autocomplete="current-password"
          :placeholder="t('login.passwordPlaceholder')"
        />
      </label>

      <n-button type="primary" block attr-type="submit" :loading="account.loading">
        {{ t('login.submit') }}
      </n-button>
      <p v-if="account.error" class="err">{{ account.error }}</p>
      <!-- 没有账号时引导到官网注册页（系统浏览器打开） -->
      <p class="register-hint">
        {{ t('login.goRegisterSplit') }}
        <a role="button" tabindex="0" @click="goRegister" @keydown.enter="goRegister">
          {{ t('login.goRegister') }}
        </a>
      </p>

      <div class="divider"><span>{{ t('login.telegramOr') }}</span></div>

      <!-- Telegram 一键登录：challenge 由控制面签发，本页轮询换票 -->
      <div v-if="!tgChallenge" class="tg-entry">
        <button type="button" class="tg-btn" :disabled="tgStarting" @click="startTelegram">
          {{ tgStarting ? t('login.tgWaiting') : t('login.telegram') }}
        </button>
      </div>
      <div v-else class="tg-panel">
        <div v-if="!tgExpired" class="tg-qr" v-html="tgChallenge.qrSvg"></div>
        <p class="tg-hint">
          {{ tgExpired ? t('login.tgExpired') : t('login.tgScanHint') }}
          <template v-if="!tgExpired">（{{ tgCountdown }}）</template>
        </p>
        <button v-if="!tgExpired" type="button" class="tg-btn" @click="openTelegram">
          {{ t('login.tgOpen') }}
        </button>
        <button v-else type="button" class="tg-btn" @click="startTelegram">
          {{ t('login.tgRefresh') }}
        </button>
        <p class="tg-status" :class="{ err: tgFailed }">{{ tgStatusText }}</p>
        <a class="tg-back" role="button" tabindex="0" @click="cancelTelegram" @keydown.enter="cancelTelegram">
          {{ t('login.tgBack') }}
        </a>
      </div>
    </form>
  </div>
</template>

<script setup lang="ts">
import { computed, onUnmounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { openUrl } from '@tauri-apps/plugin-opener'
import { usePikaAccountStore } from '@/stores/pika/AccountStore'
import {
  controlPlaneBase,
  pollPikaTelegramLogin,
  startPikaTelegramLogin,
  type PikaTelegramChallenge,
} from '@/services/pika-account-service'

const { t } = useI18n()
const router = useRouter()
const account = usePikaAccountStore()
const email = ref('')
const password = ref('')

// 跳到控制面官网的注册页（Xboard SPA 的 /#/register），系统浏览器打开
const goRegister = async () => {
  try {
    await openUrl(`${controlPlaneBase()}/#/register`)
  } catch {
    /* 打开失败时静默，登录不受影响 */
  }
}

const tgChallenge = ref<PikaTelegramChallenge | null>(null)
const tgStarting = ref(false)
const tgStatusText = ref('')
const tgFailed = ref(false)
const tgNow = ref(Date.now())
let tgPollTimer: ReturnType<typeof setTimeout> | undefined
let tgTickTimer: ReturnType<typeof setInterval> | undefined
let tgUnavailableCount = 0

const tgExpired = computed(() =>
  Boolean(tgChallenge.value) && tgNow.value >= (tgChallenge.value?.expiresAt ?? 0),
)
const tgCountdown = computed(() => {
  const remain = Math.max(0, Math.floor(((tgChallenge.value?.expiresAt ?? 0) - tgNow.value) / 1000))
  const m = Math.floor(remain / 60)
  const s = remain % 60
  return `${m}:${s < 10 ? '0' : ''}${s}`
})

const stopTelegramTimers = () => {
  if (tgPollTimer) clearTimeout(tgPollTimer)
  if (tgTickTimer) clearInterval(tgTickTimer)
  tgPollTimer = undefined
  tgTickTimer = undefined
}

const openTelegram = async () => {
  if (!tgChallenge.value) return
  try {
    await openUrl(tgChallenge.value.qrUrl)
  } catch {
    /* 打开失败时用户仍可扫码 */
  }
}

const finishTelegram = async (authData: string, loginHint: string) => {
  stopTelegramTimers()
  tgStatusText.value = t('login.tgConfirmed')
  try {
    await account.loginWithTelegram(authData, loginHint)
    if (account.loggedIn) {
      await router.replace('/')
    }
  } catch {
    tgChallenge.value = null
  }
}

const pollTelegram = async () => {
  const challenge = tgChallenge.value
  if (!challenge) return
  if (Date.now() >= challenge.expiresAt) {
    stopTelegramTimers()
    tgNow.value = Date.now()
    return
  }
  let result
  try {
    result = await pollPikaTelegramLogin(challenge.challengeId, challenge.opaqueCode)
  } catch (err) {
    stopTelegramTimers()
    tgFailed.value = true
    tgStatusText.value = err instanceof Error ? err.message : t('login.tgUnavailable')
    return
  }
  if (result.status === 'confirmed' && result.authData) {
    await finishTelegram(result.authData, result.loginHint)
    return
  }
  if (result.status === 'banned') {
    stopTelegramTimers()
    tgFailed.value = true
    tgStatusText.value = t('login.tgBanned')
    return
  }
  if (result.status === 'expired' || result.status === 'denied') {
    stopTelegramTimers()
    tgNow.value = Date.now()
    tgChallenge.value = { ...challenge, expiresAt: 0 }
    return
  }
  if (result.status === 'unavailable') {
    tgUnavailableCount += 1
    if (tgUnavailableCount >= 5) {
      stopTelegramTimers()
      tgFailed.value = true
      tgStatusText.value = t('login.tgUnavailable')
      return
    }
    tgStatusText.value = t('login.tgRetrying')
  } else {
    tgUnavailableCount = 0
    tgStatusText.value = t('login.tgWaiting')
  }
  tgPollTimer = setTimeout(() => void pollTelegram(), 2000)
}

const startTelegram = async () => {
  stopTelegramTimers()
  tgStarting.value = true
  tgFailed.value = false
  tgUnavailableCount = 0
  tgChallenge.value = null
  tgStatusText.value = t('login.tgWaiting')
  try {
    tgChallenge.value = await startPikaTelegramLogin()
  } catch (err) {
    tgFailed.value = true
    tgStatusText.value = err instanceof Error ? err.message : t('login.tgUnavailable')
    return
  } finally {
    tgStarting.value = false
  }
  tgNow.value = Date.now()
  tgTickTimer = setInterval(() => {
    tgNow.value = Date.now()
    if (tgExpired.value) stopTelegramTimers()
  }, 1000)
  await openTelegram()
  tgPollTimer = setTimeout(() => void pollTelegram(), 1500)
}

const cancelTelegram = () => {
  stopTelegramTimers()
  tgChallenge.value = null
  tgFailed.value = false
  tgStatusText.value = ''
}

onUnmounted(stopTelegramTimers)

const onSubmit = async () => {
  if (!email.value.trim() || !password.value) return
  try {
    await account.login(email.value, password.value)
    if (account.loggedIn) {
      await router.replace('/')
    }
  } catch {
    /* 错误文案在 store */
  }
}
</script>

<style scoped>
.login-shell {
  position: fixed;
  inset: 0;
  z-index: 50;
  min-height: 100vh;
  width: 100%;
  display: grid;
  place-items: center;
  background: #eef2f7;
  color: #0f172a;
}
.login-card {
  width: min(420px, calc(100vw - 48px));
  padding: 32px 28px;
  border-radius: 18px;
  background: #ffffff;
  box-shadow: 0 16px 40px rgba(15, 23, 42, 0.12);
}
h1 {
  margin: 0 0 8px;
  font-size: 32px;
  font-weight: 800;
  color: #111827;
  letter-spacing: -0.03em;
}
.lead {
  margin: 0 0 24px;
  color: #334155;
  font-size: 15px;
  line-height: 1.6;
}
.field {
  display: block;
  margin: 0 0 16px;
}
.field span {
  display: block;
  margin-bottom: 6px;
  color: #1f2937;
  font-size: 14px;
  font-weight: 600;
}
.field input {
  display: block;
  width: 100%;
  height: 44px;
  box-sizing: border-box;
  padding: 0 14px;
  border: 1px solid rgba(148, 163, 184, 0.4);
  border-radius: 10px;
  background: #fff;
  color: #111827;
  -webkit-text-fill-color: #111827;
  caret-color: #111827;
  font-size: 15px;
  outline: none;
}
.field input::placeholder {
  color: #64748b;
  -webkit-text-fill-color: #64748b;
}
.field input:focus {
  border-color: #4f46e5;
  box-shadow: 0 0 0 3px rgba(79, 70, 229, 0.15);
}
.err {
  margin-top: 12px;
  color: #b42318;
}
.register-hint {
  margin: 16px 0 0;
  text-align: center;
  color: #64748b;
  font-size: 13px;
}
.register-hint a {
  color: #4f46e5;
  font-weight: 600;
  text-decoration: none;
  cursor: pointer;
}
.register-hint a:hover,
.register-hint a:focus-visible {
  text-decoration: underline;
  outline: none;
}
.divider {
  display: flex;
  align-items: center;
  gap: 12px;
  margin: 20px 0 16px;
  color: #94a3b8;
  font-size: 12px;
}
.divider::before,
.divider::after {
  content: '';
  flex: 1;
  height: 1px;
  background: rgba(148, 163, 184, 0.35);
}
.tg-entry {
  text-align: center;
}
.tg-btn {
  display: block;
  width: 100%;
  height: 42px;
  border: 1px solid rgba(79, 70, 229, 0.45);
  border-radius: 10px;
  background: #ffffff;
  color: #4f46e5;
  font-size: 14px;
  font-weight: 700;
  cursor: pointer;
}
.tg-btn:hover {
  background: rgba(79, 70, 229, 0.06);
}
.tg-btn:disabled {
  opacity: 0.6;
  cursor: default;
}
.tg-panel {
  text-align: center;
}
.tg-qr {
  width: 180px;
  height: 180px;
  margin: 0 auto 12px;
  padding: 8px;
  border: 1px solid rgba(148, 163, 184, 0.35);
  border-radius: 12px;
  background: #ffffff;
}
.tg-qr :deep(svg) {
  width: 100%;
  height: 100%;
  display: block;
}
.tg-hint {
  margin: 0 0 12px;
  color: #334155;
  font-size: 13px;
  line-height: 1.6;
}
.tg-status {
  margin: 12px 0 0;
  color: #334155;
  font-size: 13px;
}
.tg-status.err {
  color: #b42318;
}
.tg-back {
  display: inline-block;
  margin-top: 10px;
  color: #4f46e5;
  font-size: 13px;
  font-weight: 600;
  text-decoration: none;
  cursor: pointer;
}
.tg-back:hover,
.tg-back:focus-visible {
  text-decoration: underline;
  outline: none;
}
:deep(.n-button),
:deep(.n-button .n-button__content) {
  color: #ffffff !important;
  font-weight: 700;
}
:deep(.n-button.n-button--primary-type) {
  background: #4f46e5 !important;
  --n-color: #4f46e5;
  --n-color-hover: #4338ca;
  --n-color-pressed: #3730a3;
  --n-text-color: #ffffff;
  --n-text-color-hover: #ffffff;
  --n-text-color-pressed: #ffffff;
  --n-text-color-focus: #ffffff;
}
</style>
