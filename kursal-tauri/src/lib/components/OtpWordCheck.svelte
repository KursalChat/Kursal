<script lang="ts">
  import { slide } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import { CircleAlert } from 'lucide-svelte';
  import { t } from '$lib/i18n';

  interface Props {
    words: string[];
    issues: (string[] | null)[];
    onpick: (index: number, word: string) => void;
    onedit: (index: number, word: string) => void;
  }

  let { words, issues, onpick, onedit }: Props = $props();

  function handleWordInput(index: number, el: HTMLInputElement) {
    const clean = el.value.toLowerCase().replace(/[^a-z]/g, '');
    if (el.value !== clean) {
      const caret = Math.max(
        0,
        (el.selectionStart ?? clean.length) - (el.value.length - clean.length)
      );
      el.value = clean;
      el.setSelectionRange(caret, caret);
    }
    onedit(index, clean);
  }

  const fixes = $derived(
    words
      .map((word, index) => ({ word, index, suggestions: issues[index] }))
      .filter((fix) => fix.suggestions !== null && fix.suggestions !== undefined)
  );
</script>

<div class="check" transition:slide={{ duration: 180, easing: cubicOut }}>
  <ol class="word-grid" aria-label={t('addContact.otp.enteredWordsAriaLabel')}>
    {#each words as word, i}
      <li class:bad={issues[i] != null}>
        <span class="word-index">{(i + 1).toString().padStart(2, '0')}</span>
        <input
          class="word-value"
          value={word}
          oninput={(e) => handleWordInput(i, e.currentTarget)}
          aria-label={t('addContact.otp.wordAriaLabel', { n: i + 1 })}
          autocapitalize="off"
          autocomplete="off"
          autocorrect="off"
          spellcheck="false"
        />
        {#if issues[i] != null}
          <CircleAlert size={13} />
        {/if}
      </li>
    {/each}
  </ol>

  {#each fixes as fix (fix.index)}
    <div class="fix">
      <span class="fix-word">{fix.word}</span>
      {#if fix.suggestions?.length}
        <div class="suggestions">
          {#each fix.suggestions as suggestion}
            <button
              type="button"
              onclick={() => onpick(fix.index, suggestion)}
              aria-label={t('addContact.otp.replaceWith', { word: suggestion })}
            >
              {suggestion}
            </button>
          {/each}
        </div>
      {:else}
        <span class="no-fix">{t('addContact.otp.noSuggestion')}</span>
      {/if}
    </div>
  {/each}
</div>

<style>
  .check {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .word-grid {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 6px;
  }

  .word-grid li {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 7px 9px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--bg-input);
    min-width: 0;
  }

  .word-grid li.bad {
    border-color: color-mix(in srgb, var(--danger) 55%, transparent);
    background: var(--danger-dim);
    color: var(--danger);
  }

  .word-grid li.bad :global(svg) {
    margin-left: auto;
    align-self: center;
    flex-shrink: 0;
  }

  .word-index {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--accent);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }

  .bad .word-index {
    color: inherit;
    opacity: 0.7;
  }

  .word-value {
    flex: 1;
    min-width: 0;
    border: none;
    background: none;
    padding: 0;
    color: inherit;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    font-weight: 600;
  }

  .word-value:focus {
    outline: none;
  }

  .word-grid li:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-dim);
  }

  .bad .word-value {
    text-decoration: underline;
    text-decoration-style: wavy;
    text-decoration-skip-ink: none;
    text-underline-offset: 3px;
  }

  .fix {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
  }

  .fix-word {
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 600;
    color: var(--danger);
  }

  .fix-word::after {
    content: '→';
    margin-left: 8px;
    color: var(--text-muted);
  }

  .suggestions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    min-width: 0;
  }

  .suggestions button {
    border-radius: var(--radius-sm);
    border: 1px solid color-mix(in srgb, var(--accent) 45%, transparent);
    background: var(--accent-dim);
    color: var(--accent);
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 600;
    padding: 5px 9px;
    transition:
      background var(--transition),
      color var(--transition);
  }

  .suggestions button:hover {
    background: var(--accent);
    color: var(--bg-primary);
  }

  .no-fix {
    font-size: 12px;
    color: var(--text-muted);
  }

  @media (max-width: 640px) {
    .word-grid {
      grid-template-columns: repeat(2, 1fr);
    }
  }
</style>
