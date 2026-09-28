<script lang="ts">
  import { readRaw, removeRaw, writeRaw } from '$lib/utils/storage';
  import { ONBOARDED_KEY } from '$lib/utils/storage-keys';
  import { MOCK_OS_KEY, app, controls, peer } from './backend';
  import type { ConnectionStatus } from './fixtures';

  type Action = [label: string, run: (contactId: string) => void];

  const PANEL_OPEN_KEY = 'kursal_mock_panel_open';
  const STATUSES: ConnectionStatus[] = [
    'connecting',
    'relay',
    'holepunch',
    'direct',
    'disconnected',
  ];
  const OSES = ['macos', 'windows', 'linux', 'android', 'ios'];

  const incoming: Action[] = [
    ['Message', (id) => peer.message(id)],
    ['Mailbox (3h old)', peer.mailbox],
    ['Backdated (6 months)', peer.backdated],
    ['Future-dated (+1 day)', peer.futureDated],
    ['Burst ×20', peer.burst],
    ['File offer', peer.fileOffer],
    ['Offline gap', peer.gap],
  ];

  const peerActions: Action[] = [
    ['Typing', peer.typing],
    ['Reads all', peer.readAll],
    ['Reacts', peer.react],
    ['Edits last', peer.editLast],
    ['Deletes last', peer.deleteLast],
    ['Renames', peer.rename],
    ['Calls you', peer.call],
    ['Toggle terminated', peer.toggleTerminated],
  ];

  let open = $state(readRaw(PANEL_OPEN_KEY) === '1');
  let target = $state('auto');
  let contacts = $state(peer.contacts());
  let online = $state(true);
  let syncing = $state(false);
  let autoReply = $state(controls.autoReply);
  let latency = $state(String(controls.latencyMs));
  let os = $state(readRaw(MOCK_OS_KEY) ?? 'macos');

  function toggle() {
    open = !open;
    writeRaw(PANEL_OPEN_KEY, open ? '1' : '0');
    contacts = peer.contacts();
  }

  function targetId(): string | null {
    if (target !== 'auto') return target;
    const match = location.pathname.match(/^\/chat\/([^/?#]+)/);
    return match?.[1] ?? contacts[0]?.id ?? null;
  }

  function run(action: (contactId: string) => void) {
    const id = targetId();
    if (id) action(id);
  }

  function setOs(value: string) {
    writeRaw(MOCK_OS_KEY, value);
    location.reload();
  }

  function resetOnboarding() {
    removeRaw(ONBOARDED_KEY);
    location.assign('/');
  }
</script>

<button class="tab" onclick={toggle}>MOCK</button>

{#if open}
  <aside class="panel">
    <label class="row">
      <span>Target</span>
      <select bind:value={target} onfocus={() => (contacts = peer.contacts())}>
        <option value="auto">Open chat</option>
        {#each contacts as c (c.id)}
          <option value={c.id}>{c.name}</option>
        {/each}
      </select>
    </label>

    <h4>Incoming</h4>
    <div class="grid">
      {#each incoming as [label, action] (label)}
        <button onclick={() => run(action)}>{label}</button>
      {/each}
    </div>

    <h4>Peer</h4>
    <div class="grid">
      {#each peerActions as [label, action] (label)}
        <button onclick={() => run(action)}>{label}</button>
      {/each}
      <button onclick={() => peer.hangupAll('hangup')}>Hangs up</button>
      <button onclick={() => peer.hangupAll('timeout')}>Call times out</button>
    </div>

    <h4>Connection</h4>
    <div class="grid">
      {#each STATUSES as status (status)}
        <button onclick={() => run((id) => peer.setConnection(id, status))}>{status}</button>
      {/each}
      <button onclick={() => peer.addContact()}>New contact</button>
    </div>

    <h4>App</h4>
    <label class="row">
      <span>Network online</span>
      <input type="checkbox" bind:checked={online} onchange={() => app.network(online)} />
    </label>
    <label class="row">
      <span>Offline sync</span>
      <input type="checkbox" bind:checked={syncing} onchange={() => app.offlineSync(syncing)} />
    </label>
    <div class="grid">
      <button onclick={app.incomingError}>Incoming error</button>
      <button onclick={app.nearbyRequest}>Nearby request</button>
      <button onclick={app.updateDownload}>Update download</button>
      <button onclick={resetOnboarding}>Onboarding</button>
    </div>

    <h4>Environment</h4>
    <label class="row">
      <span>Auto-reply</span>
      <input
        type="checkbox"
        bind:checked={autoReply}
        onchange={() => (controls.autoReply = autoReply)}
      />
    </label>
    <label class="row">
      <span>IPC latency</span>
      <select bind:value={latency} onchange={() => (controls.latencyMs = Number(latency))}>
        <option value="0">0 ms</option>
        <option value="300">300 ms</option>
        <option value="1500">1.5 s</option>
      </select>
    </label>
    <label class="row">
      <span>OS</span>
      <select bind:value={os} onchange={() => setOs(os)}>
        {#each OSES as value (value)}
          <option {value}>{value}</option>
        {/each}
      </select>
    </label>
  </aside>
{/if}

<style>
  .tab,
  .panel {
    position: fixed;
    z-index: 2147483000;
    font:
      11px/1.3 ui-monospace,
      SFMono-Regular,
      Menlo,
      monospace;
    color: #e5e7eb;
    background: rgba(17, 24, 39, 0.94);
    border: 1px solid #374151;
  }
  .tab {
    right: 0;
    top: 40%;
    padding: 8px 4px;
    writing-mode: vertical-rl;
    border-right: none;
    border-radius: 6px 0 0 6px;
    cursor: pointer;
    opacity: 0.6;
  }
  .tab:hover {
    opacity: 1;
  }
  .panel {
    right: 26px;
    top: 50%;
    transform: translateY(-50%);
    width: 250px;
    max-height: 90vh;
    overflow-y: auto;
    padding: 10px;
    border-radius: 8px;
    box-shadow: 0 8px 30px rgba(0, 0, 0, 0.4);
  }
  h4 {
    margin: 10px 0 4px;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: #9ca3af;
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin: 4px 0;
  }
  button:not(.tab),
  select {
    font: inherit;
    color: inherit;
    background: #1f2937;
    border: 1px solid #374151;
    border-radius: 4px;
    padding: 4px 6px;
    cursor: pointer;
    text-align: left;
  }
  button:not(.tab):hover {
    background: #374151;
  }
  select {
    max-width: 140px;
  }
</style>
