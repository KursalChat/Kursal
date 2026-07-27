<script lang="ts">
  let { size = 20, color = 'var(--accent)' }: { size?: number; color?: string } = $props();
</script>

<span class="spinner" style="width:{size}px;height:{size}px;border-top-color:{color}"></span>

<style>
  .spinner {
    border-radius: 50%;
    border: 2px solid var(--bg-tertiary);
    border-top: 2px solid currentColor;
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
