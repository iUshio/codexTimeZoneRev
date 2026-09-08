<script setup lang="ts">
defineProps<{
  name: 'clock' | 'copy' | 'check' | 'refresh' | 'folder' | 'info' | 'alert'
  size?: number
}>()
</script>

<template>
  <svg
    class="app-icon-svg"
    :width="size || 16"
    :height="size || 16"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.75"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <template v-if="name === 'clock'">
      <circle cx="12" cy="12" r="8.5" />
      <path d="M12 7.5v5l3.4 2" />
    </template>
    <template v-else-if="name === 'copy'">
      <rect x="8" y="8" width="11" height="11" rx="1.8" />
      <path d="M16 8V5.8A1.8 1.8 0 0 0 14.2 4H5.8A1.8 1.8 0 0 0 4 5.8v8.4A1.8 1.8 0 0 0 5.8 16H8" />
    </template>
    <template v-else-if="name === 'check'">
      <path d="m5 12.5 4.2 4.2L19 7" />
    </template>
    <template v-else-if="name === 'refresh'">
      <path d="M19 8a7.5 7.5 0 0 0-12.8-2L4 8.3" />
      <path d="M4 4.5v3.8h3.8M5 16a7.5 7.5 0 0 0 12.8 2l2.2-2.3" />
      <path d="M20 19.5v-3.8h-3.8" />
    </template>
    <template v-else-if="name === 'folder'">
      <path d="M3.5 7.2A2.2 2.2 0 0 1 5.7 5h4l2 2h6.6a2.2 2.2 0 0 1 2.2 2.2v7.3a2.5 2.5 0 0 1-2.5 2.5H6a2.5 2.5 0 0 1-2.5-2.5Z" />
    </template>
    <template v-else-if="name === 'info'">
      <circle cx="12" cy="12" r="8.5" />
      <path d="M12 10.7v5M12 7.7h.01" />
    </template>
    <template v-else>
      <path d="M10.3 4.5 3.2 17a2 2 0 0 0 1.7 3h14.2a2 2 0 0 0 1.7-3L13.7 4.5a2 2 0 0 0-3.4 0Z" />
      <path d="M12 9v4.5M12 17h.01" />
    </template>
  </svg>
</template>
