import type { ObjectDirective } from 'vue'

const focusableSelector = [
  'button:not([disabled])',
  'a[href]',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',')

const previousFocusByDialog = new WeakMap<HTMLElement, HTMLElement | null>()
const keydownHandlerByDialog = new WeakMap<HTMLElement, (event: KeyboardEvent) => void>()
const previousOverflowByDialog = new WeakMap<HTMLElement, string>()

export function wrappedDialogFocusIndex(
  currentIndex: number,
  itemCount: number,
  shiftKey: boolean,
): number | null {
  if (itemCount <= 0) return null
  if (currentIndex < 0) return shiftKey ? itemCount - 1 : 0
  if (shiftKey && currentIndex === 0) return itemCount - 1
  if (!shiftKey && currentIndex === itemCount - 1) return 0
  return null
}

export function getDialogFocusableElements(dialog: HTMLElement): HTMLElement[] {
  return Array.from(dialog.querySelectorAll<HTMLElement>(focusableSelector))
    .filter((element) => element.getAttribute('aria-hidden') !== 'true')
}

export const vFocusTrap: ObjectDirective<HTMLElement> = {
  mounted(dialog) {
    const previousFocus = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null
    previousFocusByDialog.set(dialog, previousFocus)
    previousOverflowByDialog.set(dialog, document.body.style.overflow)
    document.body.style.overflow = 'hidden'

    const handleKeydown = (event: KeyboardEvent) => {
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

    keydownHandlerByDialog.set(dialog, handleKeydown)
    dialog.addEventListener('keydown', handleKeydown)
    queueMicrotask(() => {
      const firstFocusable = getDialogFocusableElements(dialog)[0]
      if (firstFocusable) firstFocusable.focus()
      else dialog.focus()
    })
  },

  unmounted(dialog) {
    const handleKeydown = keydownHandlerByDialog.get(dialog)
    if (handleKeydown) dialog.removeEventListener('keydown', handleKeydown)
    keydownHandlerByDialog.delete(dialog)

    const previousOverflow = previousOverflowByDialog.get(dialog)
    previousOverflowByDialog.delete(dialog)
    document.body.style.overflow = previousOverflow ?? ''

    const previousFocus = previousFocusByDialog.get(dialog)
    previousFocusByDialog.delete(dialog)
    if (previousFocus?.isConnected) previousFocus.focus()
  },
}
