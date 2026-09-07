<template>
  <div class="page-shell">
    <PageHeader title="Pika 账号" subtitle="登录后自动下发官方线路，自己导入的订阅仍在订阅页管理。" />
    <n-card v-if="account.loggedIn">
      <p class="id">{{ account.session?.identifier }}</p>
      <p class="muted">{{ account.planHint }}</p>
      <p v-if="account.session" class="quota">
        {{ t('home.quotaUsed') }} {{ formatBytes(account.usedBytes) }}
        ·
        {{ t('home.quotaRemaining') }}
        {{ account.session.totalBytes > 0 ? formatBytes(account.remainingBytes) : t('home.quotaUnlimited') }}
      </p>
      <n-space style="margin-top: 16px">
        <n-button type="primary" :loading="account.loading" @click="onRefresh">刷新官方线路</n-button>
        <n-button :loading="loggingOut" @click="onLogout">退出登录</n-button>
      </n-space>
    </n-card>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useMessage } from 'naive-ui'
import PageHeader from '@/components/common/PageHeader.vue'
import { useI18n } from 'vue-i18n'
import { usePikaAccountStore } from '@/stores/pika/AccountStore'
import { formatBytes } from '@/utils'

const { t } = useI18n()
const router = useRouter()
const message = useMessage()
const account = usePikaAccountStore()
const loggingOut = ref(false)

const onRefresh = async () => {
  try {
    await account.refresh()
    message.success('官方线路已更新')
  } catch (error) {
    message.error(error instanceof Error ? error.message : '刷新失败')
  }
}

const onLogout = async () => {
  if (loggingOut.value) return
  loggingOut.value = true
  try {
    await account.logout()
    await router.replace('/login')
  } finally {
    loggingOut.value = false
  }
}
</script>

<style scoped>
.id {
  margin: 0 0 6px;
  font-size: 18px;
  font-weight: 700;
}
.muted {
  margin: 0;
  color: #667085;
}
.quota {
  margin: 8px 0 0;
  color: #334155;
  font-size: 14px;
}
</style>
