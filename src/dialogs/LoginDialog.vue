<script setup lang="ts">
import { ref } from 'vue'
import { commands } from '../bindings.ts'
import { useMessage } from 'naive-ui'
import FloatLabelInput from '../components/FloatLabelInput.vue'

const message = useMessage()


const showing = defineModel<boolean>('showing', { required: true })

const usernameInput = ref<string>('')
const passwordInput = ref<string>('')

async function onLogin(username: string, password: string) {
  if (username === '') {
    message.error('请输入用户名')
    return
  }
  if (password === '') {
    message.error('请输入密码')
    return
  }
  // jm 登录成功后返回的是用户名，不是可当 Authorization 用的 token。
  // 因此这里只提示登录成功，不再写 store.config.token。
  const result = await commands.login(username, password)
  if (result.status === 'error') {
    console.error(result.error)
    message.error(result.error.err_message)
    return
  }
  message.success('登录成功')
  showing.value = false
}
</script>

<template>
  <n-modal v-model:show="showing">
    <n-dialog
      :showIcon="false"
      title="账号登录"
      positive-text="登录"
      @positive-click="onLogin(usernameInput, passwordInput)"
      @keydown.enter="onLogin(usernameInput, passwordInput)"
      @close="showing = false">
      <div class="flex flex-col gap-2">
        <FloatLabelInput label="用户名" v-model:value="usernameInput" />
        <FloatLabelInput label="密码" v-model:value="passwordInput" type="password" />
      </div>
    </n-dialog>
  </n-modal>
</template>
