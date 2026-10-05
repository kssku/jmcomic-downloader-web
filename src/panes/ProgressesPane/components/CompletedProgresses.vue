<script setup lang="ts">
import { ProgressData } from '../../../types.ts'
import { computed } from 'vue'
import { useStore } from '../../../store.ts'

const store = useStore()

const completedProgresses = computed<[string, ProgressData][]>(() =>
  Array.from(store.progresses.entries())
    .filter(([, { state }]) => state === 'Completed')
    .sort((a, b) => {
      return b[1].totalImgCount - a[1].totalImgCount
    }),
)
</script>

<template>
  <div class="h-full flex flex-col gap-row-2 px-2 overflow-auto">
    <div
      class="grid grid-cols-[1fr_1fr] py-2 px-4 rounded-lg"
      style="
        background: var(--bg-raised);
        backdrop-filter: blur(var(--glass-blur, 12px)) saturate(var(--glass-saturate, 1.2));
        -webkit-backdrop-filter: blur(var(--glass-blur, 12px)) saturate(var(--glass-saturate, 1.2));
        border: var(--border-width) solid
          color-mix(in srgb, var(--border-color) calc(var(--border-alpha) * 100%), transparent);
      "
      v-for="[chapterId, { chapterInfo, comic }] in completedProgresses"
      :key="chapterId">
      <span class="text-ellipsis whitespace-nowrap overflow-hidden" :title="comic.name">
        {{ comic.name }}
      </span>
      <span class="text-ellipsis whitespace-nowrap overflow-hidden" :title="chapterInfo.chapterTitle">
        {{ chapterInfo.chapterTitle }}
      </span>
    </div>
  </div>
</template>

