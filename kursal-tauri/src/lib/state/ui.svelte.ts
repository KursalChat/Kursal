export const uiState = $state({
  mobileSidebarOpen: false,
  // 0..1 while a swipe is dragging the mobile drawer, null when it isn't.
  sidebarDrag: null as number | null,
  // Set by the command palette's message search; consumed by the chat page to
  // jump to a specific message after navigating to the conversation.
  pendingMessageJump: null as { contactId: string; messageId: string } | null,
});
