import { mount } from 'svelte';
import { mockConvertFileSrc, mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import { readRaw, writeRaw } from '#lib/utils/storage.js';
import { ONBOARDED_KEY } from '#lib/utils/storage-keys.js';
import { MOCK_OS_KEY, app, controls, handleCommand, peer } from './backend';
import MockPanel from './MockPanel.svelte';

const MOCK_SEEDED_KEY = 'kursal_mock_seeded';

export function installMock() {
  const os = readRaw(MOCK_OS_KEY) ?? 'macos';
  Object.assign(window, {
    __TAURI_OS_PLUGIN_INTERNALS__: {
      platform: os,
      os_type: os,
      family: os === 'windows' ? 'windows' : 'unix',
      version: '0.0.0-mock',
      arch: 'aarch64',
      eol: os === 'windows' ? '\r\n' : '\n',
      exe_extension: os === 'windows' ? 'exe' : '',
    },
  });

  mockWindows('main');
  mockConvertFileSrc(os === 'windows' ? 'windows' : 'macos');
  mockIPC(handleCommand, { shouldMockEvents: true });

  if (readRaw(MOCK_SEEDED_KEY) !== 'done') {
    writeRaw(ONBOARDED_KEY, 'done');
    writeRaw(MOCK_SEEDED_KEY, 'done');
  }

  Object.assign(window, { kursalMock: { peer, app, controls } });
  mount(MockPanel, { target: document.body });
}
