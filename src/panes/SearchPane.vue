<script setup lang="ts">
import { computed, ref } from 'vue'
import { commands, SearchSort } from '../bindings.ts'
import ComicCard from '../components/ComicCard.vue'
import { PhMagnifyingGlass, PhArrowRight } from '@phosphor-icons/vue'
import FloatLabelInput from '../components/FloatLabelInput.vue'
import { useStore } from '../store.ts'
import { SelectProps } from 'naive-ui'

const store = useStore()

/** jm 搜索每页固定 80 条，用于把 total 换算成页数。 */
const PAGE_SIZE = 80

const sortOptions: SelectProps['options'] = [
  { label: '新到旧', value: 'TimeNewest' },
  { label: '旧到新', value: 'TimeOldest' },
  { label: '最多爱心', value: 'LikeMost' },
  { label: '最多指名', value: 'ViewMost' },
]

const searchInput = ref<string>('')
const searching = ref<boolean>(false)
const comicIdInput = ref<string>('')
const sortSelected = ref<SearchSort>('TimeNewest')
const currentPage = ref<number>(1)

// jm 搜索只返回 total，不返回页数；按固定页大小估算。
const pageCount = computed(() => {
  const total = store.searchResult?.total ?? 0
  return Math.max(1, Math.ceil(total / PAGE_SIZE))
})

async function searchByKeyword(keyword: string, sort: SearchSort, page: number) {
  searching.value = true
  currentPage.value = page
  // categories 参数后端已忽略，保留空数组以维持调用形状。
  const result = await commands.searchComic(keyword, sort, page, [])
  if (result.status === 'error') {
    searching.value = false
    console.error(result.error)
    return
  }
  searching.value = false
  store.searchResult = result.data
}

async function pickComic() {
  const result = await commands.getComic(comicIdInput.value.trim())
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
  store.pickedComic = result.data
  store.currentTabName = 'chapter'
}
</script>

<template>
  <div class="h-full flex flex-col gap-2">
    <n-input-group class="box-border px-2 pt-2">
      <FloatLabelInput
        label="关键词"
        size="small"
        v-model:value="searchInput"
        clearable
        @keydown.enter="searchByKeyword(searchInput.trim(), sortSelected, 1)" />
      <n-select
        class="w-45%"
        v-model:value="sortSelected"
        :options="sortOptions"
        :show-checkmark="false"
        size="small"
        @update-value="searchByKeyword(searchInput.trim(), $event, 1)" />
      <n-button
        :loading="searching"
        type="primary"
        size="small"
        class="w-15%"
        @click="searchByKeyword(searchInput.trim(), sortSelected, 1)">
        <template #icon>
          <n-icon size="22">
            <PhMagnifyingGlass />
          </n-icon>
        </template>
      </n-button>
    </n-input-group>

    <n-input-group class="box-border px-2">
      <FloatLabelInput label="漫画ID" size="small" v-model:value="comicIdInput" clearable @keydown.enter="pickComic" />
      <n-button type="primary" size="small" class="w-15%" @click="pickComic">
        <template #icon>
          <n-icon size="20">
            <PhArrowRight />
          </n-icon>
        </template>
      </n-button>
    </n-input-group>

    <div v-if="store.searchResult !== undefined" class="flex flex-col gap-row-2 overflow-auto box-border px-2">
      <comic-card
        v-for="{ id, name, author, image, isDownloaded, comicDownloadDir } in store.searchResult.docs"
        :key="id"
        :comic-id="id"
        :comic-name="name"
        :comic-author="author"
        :comic-downloaded="isDownloaded"
        :comic-download-dir="comicDownloadDir"
        :image="image" />
    </div>

    <n-pagination
      v-if="store.searchResult !== undefined"
      class="box-border p-2 pt-0 mt-auto"
      :page-count="pageCount"
      :page="currentPage"
      @update:page="searchByKeyword(searchInput.trim(), sortSelected, $event)" />
  </div>
</template>

