import {
  getLocalUserProfile,
  getLocalPeerId,
  getLocalUserId,
  setLocalUserAvatar,
  broadcastProfile,
} from '$lib/api/identity';
import { withAvatarCacheBust } from '$lib/utils/avatarUrl';
import { log } from '$lib/utils/log';

function createProfileState() {
  let displayName = $state('You');
  let avatarPath = $state<string | null>(null);
  let peerId = $state<string | null>(null);
  let userId = $state<string | null>(null);
  let loading = $state(false);
  let initialized = $state(false);

  async function load() {
    if (initialized) return;
    loading = true;
    try {
      const [storedName, storedAvatar] = await getLocalUserProfile();
      if (storedName) displayName = storedName;

      avatarPath = withAvatarCacheBust(storedAvatar || null);
    } catch (e) {
      log.error('Failed to load user profile:', e);
    }

    try {
      peerId = await getLocalPeerId();
      userId = await getLocalUserId();
    } catch (e) {
      log.error('Failed to load peer/user id:', e);
    }

    initialized = true;
    loading = false;
  }

  function update(name: string, path: string | null) {
    displayName = name;
    avatarPath = withAvatarCacheBust(path);
  }

  // `undefined` keeps the stored avatar; `null` clears it. Broadcasts the name
  // to contacts, then commits both locally, returning the cache-busted path.
  async function save(name: string, avatarBytes?: number[] | null): Promise<string | null> {
    const path = avatarBytes === undefined ? avatarPath : await setLocalUserAvatar(avatarBytes);
    const refreshedPath = withAvatarCacheBust(path);
    await broadcastProfile(name);
    update(name, refreshedPath);
    return refreshedPath;
  }

  async function refreshPeerId() {
    try {
      peerId = await getLocalPeerId();
    } catch (e) {
      log.error(e);
    }
  }

  return {
    get displayName() {
      return displayName;
    },
    get avatarPath() {
      return avatarPath;
    },
    get peerId() {
      return peerId;
    },
    get userId() {
      return userId;
    },
    get loading() {
      return loading;
    },
    load,
    update,
    save,
    refreshPeerId,
  };
}

export const profileState = createProfileState();
