import { ref, type Ref } from 'vue'

export function useBubbleTimer(hideDelayMs = 10000) {
  const bubblesVisible = ref(false)
  const inputVisible = ref(false)
  let hideTimer: ReturnType<typeof setTimeout> | null = null
  let chatLoadingRef: Ref<boolean> | null = null
  let inputFocused = false

  function showBubbles() {
    bubblesVisible.value = true
    resetHideTimer()
  }

  function hideBubbles() {
    bubblesVisible.value = false
    inputVisible.value = false
    inputFocused = false
    clearHideTimer()
  }

  function showInput() {
    inputVisible.value = true
    showBubbles()
  }

  function toggleInput() {
    if (inputVisible.value) {
      hideBubbles()
    } else {
      showInput()
    }
  }

  function scheduleHide() {
    clearHideTimer()
    if (chatLoadingRef && chatLoadingRef.value) {
      return
    }
    if (inputFocused) {
      return
    }
    hideTimer = setTimeout(() => {
      bubblesVisible.value = false
      inputVisible.value = false
      inputFocused = false
    }, hideDelayMs)
  }

  function clearHideTimer() {
    if (hideTimer !== null) {
      clearTimeout(hideTimer)
      hideTimer = null
    }
  }

  function onNewMessage() {
    showBubbles()
  }

  function onUserActivity() {
    if (bubblesVisible.value) {
      scheduleHide()
    }
  }

  function onInputFocus() {
    inputFocused = true
    clearHideTimer()
  }

  function onInputBlur() {
    inputFocused = false
    scheduleHide()
  }

  function bindChatLoading(loading: Ref<boolean>) {
    chatLoadingRef = loading
  }

  function resetHideTimer() {
    if (chatLoadingRef && chatLoadingRef.value) {
      clearHideTimer()
      return
    }
    if (inputFocused) {
      clearHideTimer()
      return
    }
    scheduleHide()
  }

  return {
    bubblesVisible,
    inputVisible,
    showBubbles,
    hideBubbles,
    showInput,
    toggleInput,
    resetHideTimer,
    onNewMessage,
    onUserActivity,
    bindChatLoading,
    onInputFocus,
    onInputBlur,
  }
}