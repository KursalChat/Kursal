import { getLocalUserProfile, getLocalPeerId, getLocalUserId } from '$lib/api/identity';
import { log } from '$lib/utils/log';
import { bytesToBase64 } from '$lib/utils/base64';

function createProfileState() {
  let displayName = $state('You');
  let avatarBase64 = $state<string | null>(null);
  let avatarBytes = $state<number[] | null>(null);
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

      if (storedAvatar && storedAvatar.length > 0) {
        avatarBytes = storedAvatar;
        avatarBase64 = bytesToBase64(storedAvatar);
      } else {
        avatarBytes = null;
        avatarBase64 = null;
      }
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

  function update(name: string, b64: string | null, bytes: number[] | null) {
    displayName = name;
    avatarBase64 = b64;
    avatarBytes = bytes;
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
    get avatarBase64() {
      return avatarBase64;
    },
    get avatarBytes() {
      return avatarBytes;
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
    refreshPeerId,
  };
}

export const profileState = createProfileState();
