export const uiState = $state({
  mobileSidebarOpen: false,
  // Set by the command palette's message search; consumed by the chat page to
  // jump to a specific message after navigating to the conversation.
  pendingMessageJump: null as { contactId: string; messageId: string } | null,
});
