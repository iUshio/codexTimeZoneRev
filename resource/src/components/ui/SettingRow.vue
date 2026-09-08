<script setup lang="ts">
import AppIcon from './AppIcon.vue'

defineProps<{ label: string; description?: string; error?: string; fieldId?: string }>()
</script>

<template>
  <div class="setting-row" :class="{ 'setting-row--error': error }">
    <div class="setting-row-copy">
      <label v-if="fieldId" class="setting-label" :for="fieldId">{{ label }}</label>
      <span v-else class="setting-label">{{ label }}</span>
      <span v-if="description" class="setting-description">{{ description }}</span>
    </div>
    <div class="setting-control"><slot /></div>
    <p v-if="error" class="field-error"><AppIcon name="alert" />{{ error }}</p>
    <slot name="after" />
  </div>
</template>
