import { getDialogFocusableElements, wrappedDialogFocusIndex } from './dialogFocus.ts'

export type ConfirmDialogKeyAction = 'cancel' | null

export function confirmDialogKeyAction(key: string): ConfirmDialogKeyAction {
  return key === 'Escape' ? 'cancel' : null
}

export function showToast(message: string, type: 'success' | 'error' | 'info' = 'info') {
  document.querySelectorAll('.toast').forEach((item) => item.remove())

  const toast = document.createElement('div')
  toast.className = `toast toast-${type}`
  toast.setAttribute('role', type === 'error' ? 'alert' : 'status')
  toast.setAttribute('aria-live', type === 'error' ? 'assertive' : 'polite')
  toast.setAttribute('aria-atomic', 'true')
  toast.style.cssText = `
    position: fixed;
    top: 20px;
    right: 20px;
    left: auto;
    display: flex;
    align-items: flex-start;
    gap: 0.75rem;
    padding: 0.85rem 1rem;
    border: 1px solid transparent;
    border-radius: var(--radius);
    color: var(--color-ink);
    font-weight: 500;
    z-index: 9999;
    max-width: 400px;
    width: max-content;
    word-break: break-word;
    box-shadow: var(--shadow-modal);
  `

  if (type === 'success') {
    toast.style.background = 'var(--color-success-soft)'
    toast.style.borderColor = 'var(--color-success)'
    toast.style.color = 'var(--color-success)'
  } else if (type === 'error') {
    toast.style.background = 'var(--color-danger-soft)'
    toast.style.borderColor = 'var(--color-danger)'
    toast.style.color = 'var(--color-danger)'
  } else {
    toast.style.background = 'var(--color-accent-soft)'
    toast.style.borderColor = 'var(--color-accent)'
    toast.style.color = 'var(--color-accent-hover)'
  }

  const text = document.createElement('span')
  text.textContent = message
  text.style.cssText = 'flex: 1; min-width: 0;'

  const closeButton = document.createElement('button')
  closeButton.type = 'button'
  closeButton.textContent = '×'
  closeButton.setAttribute('aria-label', '关闭提示')
  closeButton.style.cssText = `
    flex: 0 0 auto;
    min-width: 28px;
    min-height: 28px;
    margin: -0.25rem -0.35rem -0.25rem 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: currentColor;
    font-size: 1.25rem;
    line-height: 1;
  `

  let dismissed = false
  const dismiss = () => {
    if (dismissed) return
    dismissed = true
    toast.remove()
  }

  closeButton.addEventListener('click', dismiss)
  toast.append(text, closeButton)

  if (window.innerWidth < 640) {
    toast.style.left = '16px'
    toast.style.right = '16px'
    toast.style.maxWidth = 'none'
    toast.style.width = 'auto'
  }

  document.body.appendChild(toast)

  window.setTimeout(dismiss, type === 'error' ? 7000 : 4500)
}

export function confirmAction(message: string): Promise<boolean> {
  return new Promise((resolve) => {
    const previousFocus = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null
    const previousOverflow = document.body.style.overflow
    let settled = false

    const overlay = document.createElement('div')
    overlay.style.cssText = `
      position: fixed;
      inset: 0;
      background: var(--color-overlay);
      display: flex;
      align-items: center;
      justify-content: center;
      z-index: 9998;
      padding: 1rem;
    `

    const dialog = document.createElement('div')
    const messageId = `confirm-message-${Date.now()}`
    dialog.setAttribute('role', 'alertdialog')
    dialog.setAttribute('aria-modal', 'true')
    dialog.setAttribute('aria-labelledby', messageId)
    dialog.tabIndex = -1
    dialog.style.cssText = `
      width: min(420px, 92vw);
      background: var(--bg-card);
      border: 1px solid var(--border-input);
      border-radius: var(--radius);
      color: var(--text-strong);
      padding: 1.25rem;
      box-shadow: var(--shadow-modal);
    `

    const text = document.createElement('p')
    text.id = messageId
    text.textContent = message
    text.style.cssText = 'margin: 0 0 1.25rem; line-height: 1.6; color: var(--text-faint);'

    const actions = document.createElement('div')
    actions.style.cssText = 'display: flex; justify-content: flex-end; gap: 0.75rem; flex-wrap: wrap;'

    const cancelButton = document.createElement('button')
    cancelButton.type = 'button'
    cancelButton.textContent = '取消'
    cancelButton.style.cssText = 'border: 1px solid var(--color-rule-strong); border-radius: var(--radius-input); min-height: 40px; padding: 0.5rem 1rem; color: var(--color-ink-2); background: var(--color-paper-3); cursor: pointer;'

    const okButton = document.createElement('button')
    okButton.type = 'button'
    okButton.textContent = '确认'
    okButton.style.cssText = 'border: 1px solid var(--color-accent); border-radius: var(--radius-input); min-height: 40px; padding: 0.5rem 1rem; color: var(--color-accent-ink); background: var(--color-accent); cursor: pointer;'

    const cleanup = (result: boolean) => {
      if (settled) return
      settled = true
      dialog.removeEventListener('keydown', handleKeydown)
      overlay.remove()
      document.body.style.overflow = previousOverflow
      if (previousFocus?.isConnected) previousFocus.focus()
      resolve(result)
    }

    const handleKeydown = (event: KeyboardEvent) => {
      if (confirmDialogKeyAction(event.key) === 'cancel') {
        event.preventDefault()
        cleanup(false)
        return
      }

      if (event.key !== 'Tab') return
      const focusable = getDialogFocusableElements(dialog)
      if (focusable.length === 0) {
        event.preventDefault()
        dialog.focus()
        return
      }

      const currentIndex = focusable.indexOf(document.activeElement as HTMLElement)
      const nextIndex = wrappedDialogFocusIndex(currentIndex, focusable.length, event.shiftKey)
      if (nextIndex === null) return
      event.preventDefault()
      focusable[nextIndex]?.focus()
    }

    cancelButton.addEventListener('click', () => cleanup(false))
    okButton.addEventListener('click', () => cleanup(true))
    overlay.addEventListener('click', (event) => {
      if (event.target === overlay) cleanup(false)
    })
    dialog.addEventListener('keydown', handleKeydown)

    actions.append(cancelButton, okButton)
    dialog.append(text, actions)
    overlay.append(dialog)
    document.body.appendChild(overlay)
    document.body.style.overflow = 'hidden'
    cancelButton.focus()
  })
}
