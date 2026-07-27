import { invoke } from '@tauri-apps/api/core';
import type { ContactResponse, ContactMeta } from '$lib/types';

export const getContacts = (): Promise<ContactResponse[]> => invoke('get_contacts');

export const getSecurityCode = (contactId: string): Promise<string> =>
  invoke('get_security_code', { contactId });

export const removeContact = (contactId: string): Promise<void> =>
  invoke('remove_contact', { contactId });

export const confirmSecurityCode = (contactId: string): Promise<void> =>
  invoke('confirm_security_code', { contactId });

export const setContactBlocked = (contactId: string, value: boolean): Promise<void> =>
  invoke('set_contact_blocked', { contactId, value });

export const getContactMeta = (): Promise<ContactMeta[]> => invoke('get_contact_meta');

export const setContactMuted = (contactId: string, value: boolean): Promise<void> =>
  invoke('set_contact_muted', { contactId, value });

export const setContactAlias = (contactId: string, value: string | null): Promise<void> =>
  invoke('set_contact_alias', { contactId, value });
