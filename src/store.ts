import { defineStore } from 'pinia'
import { CurrentTabName, ProgressData } from './types.ts'
import { Comic, Config, SearchResult, ServerInfo, UserProfileDetailRespData } from './bindings'
import { ref } from 'vue'
import { ProgressesPaneTabName } from './panes/ProgressesPane/ProgressesPane.vue'

export const useStore = defineStore('store', () => {
  const config = ref<Config>()
  const serverInfo = ref<ServerInfo>()
  const userProfile = ref<UserProfileDetailRespData>()
  const pickedComic = ref<Comic>()
  const currentTabName = ref<CurrentTabName>('progresses')
  const progresses = ref<Map<string, ProgressData>>(new Map())
  const searchResult = ref<SearchResult>()
  const progressesPaneTabName = ref<ProgressesPaneTabName>('uncompleted')
  // 列表里当前选中的任务行。undefined = 未选中，右侧详情面板显示空状态。
  // 只存 chapterId，详情从 progresses 里现取，避免两处状态不同步。
  const selectedChapterId = ref<string>()

  return {
    config,
    serverInfo,
    userProfile,
    pickedComic,
    currentTabName,
    progresses,
    searchResult,
    progressesPaneTabName,
    selectedChapterId,
  }
})
