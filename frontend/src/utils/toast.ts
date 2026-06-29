export function showToast(message: string, type: 'success' | 'error' | 'info' = 'info') {
  const existing = Array.from(document.querySelectorAll('.toast'))
  existing.slice(0, Math.max(0, existing.length - 2)).forEach((item) => item.remove())

  const toast = document.createElement('div')
  toast.className = `toast toast-${type}`
  toast.textContent = message
  toast.style.cssText = `
    position: fixed;
    top: 20px;
    right: 20px;
    left: auto;
    padding: 1rem 1.5rem;
    border-radius: 8px;
    color: white;
    font-weight: 500;
    z-index: 9999;
    animation: slideIn 0.3s ease;
    max-width: 400px;
    width: max-content;
    word-wrap: break-word;
    box-shadow: 0 10px 25px rgba(0, 0, 0, 0.35);
  `

  if (type === 'success') toast.style.background = '#10b981'
  else if (type === 'error') toast.style.background = '#ef4444'
  else toast.style.background = '#2563eb'

  document.body.appendChild(toast)

  if (window.innerWidth < 640) {
    toast.style.left = '16px'
    toast.style.right = '16px'
    toast.style.maxWidth = 'none'
    toast.style.width = 'auto'
  }

  setTimeout(() => {
    toast.style.animation = 'slideOut 0.3s ease'
    setTimeout(() => toast.remove(), 300)
  }, 3000)
}

export function confirmAction(message: string): Promise<boolean> {
  return new Promise((resolve) => {
    const overlay = document.createElement('div')
    overlay.style.cssText = `
      position: fixed;
      inset: 0;
      background: rgba(0, 0, 0, 0.7);
      display: flex;
      align-items: center;
      justify-content: center;
      z-index: 9998;
      padding: 1rem;
    `

    const dialog = document.createElement('div')
    dialog.style.cssText = `
      width: min(420px, 92vw);
      background: #111827;
      border: 1px solid #374151;
      border-radius: 8px;
      color: #fff;
      padding: 1.25rem;
      box-shadow: 0 20px 45px rgba(0, 0, 0, 0.45);
    `

    const text = document.createElement('p')
    text.textContent = message
    text.style.cssText = 'margin: 0 0 1.25rem; line-height: 1.6; color: #e5e7eb;'

    const actions = document.createElement('div')
    actions.style.cssText = 'display: flex; justify-content: flex-end; gap: 0.75rem; flex-wrap: wrap;'

    const cancelButton = document.createElement('button')
    cancelButton.textContent = '取消'
    cancelButton.style.cssText = 'border: 0; border-radius: 6px; padding: 0.5rem 1rem; color: #fff; background: #4b5563; cursor: pointer;'

    const okButton = document.createElement('button')
    okButton.textContent = '确认'
    okButton.style.cssText = 'border: 0; border-radius: 6px; padding: 0.5rem 1rem; color: #fff; background: #dc2626; cursor: pointer;'

    const cleanup = (result: boolean) => {
      document.removeEventListener('keydown', handleKeydown)
      overlay.remove()
      resolve(result)
    }

    const handleKeydown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') cleanup(false)
      if (event.key === 'Enter') cleanup(true)
    }

    cancelButton.onclick = () => cleanup(false)
    okButton.onclick = () => cleanup(true)
    overlay.onclick = (event) => {
      if (event.target === overlay) cleanup(false)
    }
    document.addEventListener('keydown', handleKeydown)

    actions.append(cancelButton, okButton)
    dialog.append(text, actions)
    overlay.append(dialog)
    document.body.appendChild(overlay)
    okButton.focus()
  })
}
