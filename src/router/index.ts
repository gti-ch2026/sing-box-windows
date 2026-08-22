import { createRouter, createWebHashHistory } from 'vue-router'
import Layout from '@/components/layout/MainLayout.vue'
import { loadSession } from '@/services/pika-account-service'

const router = createRouter({
  history: createWebHashHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/login',
      name: 'Login',
      component: () => import('@/views/PikaLoginView.vue'),
      meta: { public: true },
    },
    // 空白页面 - 独立路由，用于托盘模式下减少内存占用
    {
      path: '/blank',
      name: 'Blank',
      component: () => import('@/views/BlankView.vue'),
      meta: {
        isBlankPage: true, // 标记为空白页面
      },
    },
    // 主应用布局 - 包含所有功能页面
    {
      path: '/',
      name: 'index',
      component: Layout,
      children: [
        {
          path: '/',
          name: 'Home',
          component: () => import('@/views/HomeView.vue'),
        },
        {
          path: '/account',
          name: 'Account',
          component: () => import('@/views/PikaAccountView.vue'),
        },
        {
          path: '/sub',
          name: 'Sub',
          component: () => import('@/views/SubView.vue'),
        },
        {
          path: '/proxy',
          name: 'Proxy',
          component: () => import('@/views/ProxyView.vue'),
        },
        {
          path: '/log',
          name: 'Log',
          component: () => import('@/views/LogView.vue'),
        },
        {
          path: '/setting',
          name: 'Setting',
          component: () => import('@/views/SettingView.vue'),
        },
        {
          path: '/rules',
          name: 'Rules',
          component: () => import('@/views/RulesView.vue'),
        },
        {
          path: '/connections',
          name: 'Connections',
          component: () => import('@/views/ConnectionsView.vue'),
        },
      ],
    },
  ],
})

// 未登录只能进登录页；登录后官方线路由 Pika 账号页 / 启动流程下发
router.beforeEach((to, _from, next) => {
  const loggedIn = Boolean(loadSession()?.token)
  if (!loggedIn && !to.meta.public && to.name !== 'Blank') {
    next({ name: 'Login' })
    return
  }
  if (loggedIn && to.name === 'Login') {
    next({ name: 'Home' })
    return
  }
  next()
})

export default router
