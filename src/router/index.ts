import { createRouter, createWebHashHistory } from 'vue-router'
import Layout from '@/components/layout/MainLayout.vue'
import PikaLoginView from '@/views/PikaLoginView.vue'
import HomeView from '@/views/HomeView.vue'
import { loadSession } from '@/services/pika-account-service'

const router = createRouter({
  history: createWebHashHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/login',
      name: 'Login',
      component: PikaLoginView,
      meta: { public: true },
    },
    // 托盘占位页不能再给用户看：启动时切过来就是一整块深蓝。
    {
      path: '/blank',
      redirect: '/',
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
          component: HomeView,
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
  if (!loggedIn && to.name !== 'Login') {
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
