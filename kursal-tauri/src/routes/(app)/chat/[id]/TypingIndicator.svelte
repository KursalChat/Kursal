<script lang="ts">
  import { t } from '$lib/i18n';
  import Avatar from '$lib/components/Avatar.svelte';

  interface Props {
    name: string;
    avatar: string | null | undefined;
  }

  let { name, avatar }: Props = $props();
</script>

<div class="typing-row" aria-label={t('chat.composer.typingIndicator', { name })}>
  <Avatar {name} src={avatar} size={22} />
  <span class="typing-dots">
    <span></span><span></span><span></span>
  </span>
</div>

<style>
  .typing-row {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    margin: 6px 0 0 4px;
    padding: 4px 10px 4px 4px;
    border-radius: var(--radius-md);
    align-self: flex-start;
    background: var(--bg-hover);
    animation: typing-in 220ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .typing-dots {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--text-muted);
  }
  .typing-dots span {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: currentColor;
    animation: typing-bounce 1.2s ease-in-out infinite;
  }
  .typing-dots span:nth-child(2) {
    animation-delay: 0.16s;
  }
  .typing-dots span:nth-child(3) {
    animation-delay: 0.32s;
  }
  @keyframes typing-bounce {
    0%,
    60%,
    100% {
      transform: translateY(0);
      opacity: 0.45;
    }
    30% {
      transform: translateY(-2px);
      opacity: 1;
    }
  }
  @keyframes typing-in {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
