import { ref } from 'vue'
import { apiUrl, authHeaders, request, responseData } from '../utils/api'
import { showToast } from '../utils/toast'

export function useUsers(isAdminCheck?: () => boolean) {
  const allUsers = ref<{ id: string; username: string }[]>([])
  const usersLoading = ref(false)

  const loadUsers = async () => {
    if (isAdminCheck && !isAdminCheck()) return
    usersLoading.value = true
    try {
      const res = await request(apiUrl('/admin/users?scope=all'), { headers: authHeaders() })
      allUsers.value = await responseData<{ id: string; username: string }[]>(res)
    } catch {
      showToast('加载用户列表失败', 'error')
    } finally {
      usersLoading.value = false
    }
  }

  return { allUsers, usersLoading, loadUsers }
}
