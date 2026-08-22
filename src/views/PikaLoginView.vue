<template>
  <div class="login-shell">
    <div class="login-card">
      <h1>{{ t('login.brand') }}</h1>
      <p class="lead">{{ t('login.lead') }}</p>
      <n-form @submit.prevent="onSubmit">
        <n-form-item :label="t('login.account')">
          <n-input v-model:value="email" :placeholder="t('login.accountPlaceholder')" @keyup.enter="onSubmit" />
        </n-form-item>
        <n-form-item :label="t('login.password')">
          <n-input
            v-model:value="password"
            type="password"
            show-password-on="click"
            :placeholder="t('login.passwordPlaceholder')"
            @keyup.enter="onSubmit"
          />
        </n-form-item>
        <n-button type="primary" block :loading="account.loading" @click="onSubmit">
          {{ t('login.submit') }}
        </n-button>
        <p v-if="account.error" class="err">{{ account.error }}</p>
      </n-form>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { usePikaAccountStore } from '@/stores/pika/AccountStore'

const { t } = useI18n()
const router = useRouter()
const account = usePikaAccountStore()
const email = ref('')
const password = ref('')

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
  min-height: 100vh;
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
.err {
  margin-top: 12px;
  color: #b42318;
}
:deep(.n-form-item-label) {
  color: #1f2937 !important;
  font-weight: 600;
}
:deep(.n-input .n-input__input-el),
:deep(.n-input input),
:deep(.n-input .n-input-wrapper) {
  color: #111827 !important;
  -webkit-text-fill-color: #111827 !important;
  caret-color: #111827;
}
:deep(.n-input .n-input__placeholder),
:deep(.n-input .n-input__placeholder span) {
  color: #475569 !important;
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
