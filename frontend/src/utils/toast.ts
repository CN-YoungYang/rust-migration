export function showToast(message: string, type: 'success' | 'error' | 'info' = 'info') {
  document.querySelectorAll('.toast').forEach((item) => item.remove())

  const toast = document.createElement('div')
  toast.className = `toast toast-${type}`
  toast.setAttribute('role', type === 'error' ? 'alert' : 'status')
  toast.setAttribute('aria-live', type === 'error' ? 'assertive' : 'polite')
  toast.setAttribute('aria-atomic', 'true')
  const text = document.createElement('span')
  text.textContent = message
  text.className = 'toast__message'

  const closeButton = document.createElement('button')
  closeButton.type = 'button'
  closeButton.textContent = '×'
  closeButton.className = 'toast__close'
  closeButton.setAttribute('aria-label', '关闭提示')

  let dismissed = false
  const dismiss = () => {
    if (dismissed) return
    dismissed = true
    toast.remove()
  }

  closeButton.addEventListener('click', dismiss)
  toast.append(text, closeButton)
  document.body.appendChild(toast)

  window.setTimeout(dismiss, type === 'error' ? 7000 : 4500)
}

export function confirmAction(message: string): Promise<boolean> {
  return new Promise((resolve) => {
    const dialog = document.createElement('dialog')
    dialog.className = 'confirm-dialog'

    const form = document.createElement('form')
    form.method = 'dialog'
    form.className = 'confirm-dialog__form'

    const text = document.createElement('p')
    text.className = 'confirm-dialog__message'
    text.textContent = message

    const actions = document.createElement('div')
    actions.className = 'confirm-dialog__actions'

    const cancelButton = document.createElement('button')
    cancelButton.type = 'submit'
    cancelButton.value = 'cancel'
    cancelButton.className = 'confirm-dialog__button'
    cancelButton.textContent = '取消'

    const confirmButton = document.createElement('button')
    confirmButton.type = 'submit'
    confirmButton.value = 'confirm'
    confirmButton.className = 'confirm-dialog__button confirm-dialog__button--danger'
    confirmButton.textContent = '确认'

    dialog.addEventListener('close', () => {
      resolve(dialog.returnValue === 'confirm')
      dialog.remove()
    }, { once: true })

    actions.append(cancelButton, confirmButton)
    form.append(text, actions)
    dialog.append(form)
    document.body.appendChild(dialog)
    dialog.showModal()
    cancelButton.focus()
  })
}
