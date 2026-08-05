import { ref } from 'vue'
import { useMessage } from 'naive-ui'
import { apiUrl, request, responseData } from '../utils/api'

export function useUsers(isAdminCheck?: () => boolean) {
  const message = useMessage()
  const allUsers = ref<{ id: string; username: string }[]>([])
  const usersLoading = ref(false)

  const loadUsers = async () => {
    if (isAdminCheck && !isAdminCheck()) return
    usersLoading.value = true
    try {
      const res = await request(apiUrl('/admin/users?scope=all'))
      allUsers.value = await responseData<{ id: string; username: string }[]>(res)
    } catch {
      message.error('加载用户列表失败')
    } finally {
      usersLoading.value = false
    }
  }

  return { allUsers, usersLoading, loadUsers }
}
