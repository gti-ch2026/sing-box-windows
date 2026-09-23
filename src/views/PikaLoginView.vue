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
    </form>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { openUrl } from '@tauri-apps/plugin-opener'
import { usePikaAccountStore } from '@/stores/pika/AccountStore'
import { controlPlaneBase } from '@/services/pika-account-service'

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
