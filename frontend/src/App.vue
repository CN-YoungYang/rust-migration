<template>
  <n-config-provider
    :theme="isDark ? darkTheme : null"
    :locale="zhCN"
    :date-locale="dateZhCN"
  >
    <n-message-provider placement="top">
      <n-dialog-provider>
        <n-notification-provider>
          <n-loading-bar-provider>
            <AppShell :is-dark="isDark" @toggle-dark="toggleDark" />
          </n-loading-bar-provider>
        </n-notification-provider>
      </n-dialog-provider>
    </n-message-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import {
  NConfigProvider,
  NMessageProvider,
  NDialogProvider,
  NNotificationProvider,
  NLoadingBarProvider,
  zhCN,
  dateZhCN,
  darkTheme,
} from 'naive-ui'
import AppShell from './AppShell.vue'

const THEME_KEY = 'ai-hub-theme'

function initialDark(): boolean {
  const stored = localStorage.getItem(THEME_KEY)
  if (stored === 'dark') return true
  if (stored === 'light') return false
  return window.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false
}

const isDark = ref(initialDark())

function toggleDark() {
  isDark.value = !isDark.value
  localStorage.setItem(THEME_KEY, isDark.value ? 'dark' : 'light')
}
</script>
