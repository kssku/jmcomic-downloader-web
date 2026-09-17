<script setup lang="ts">
import { commands } from '../bindings.ts'
import { useStore } from '../store.ts'
import IconButton from './IconButton.vue'
import { PhDownloadSimple } from '@phosphor-icons/vue'

const store = useStore()

const props = defineProps<{
  comicId: string
  comicName: string
  comicAuthor: string
  comicDownloaded: boolean
  comicDownloadDir: string
  /** jm 封面完整 URL。可能为空（搜索命中单本 redirect 时后端不填）。 */
  image: string
}>()

async function downloadComic() {
  const result = await commands.downloadComic(props.comicId)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
}

async function pickComic() {
  const result = await commands.getComic(props.comicId)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
  store.pickedComic = result.data
  store.currentTabName = 'chapter'
}
</script>

<template>
  <n-card content-style="padding: 0.25rem;" hoverable>
    <div class="flex">
      <img
        class="w-24 aspect-[3/4] object-contain mr-4 cursor-pointer transform transition-transform duration-200 hover:scale-106"
        :src="image"
        alt=""
        referrerpolicy="no-referrer"
        @click="pickComic" />
      <div class="flex flex-col w-full justify-between">
        <div class="flex flex-col">
          <span
            class="font-bold text-lg line-clamp-2 cursor-pointer transition-colors duration-200 hover:text-blue-5"
            @click="pickComic">
            {{ comicName }}
          </span>
          <span class="text-red">作者：{{ comicAuthor }}</span>
        </div>
        <div class="flex">
          <IconButton class="ml-auto" title="一键下载所有章节" @click="downloadComic">
            <PhDownloadSimple :size="24" />
          </IconButton>
        </div>
      </div>
    </div>
  </n-card>
</template>
