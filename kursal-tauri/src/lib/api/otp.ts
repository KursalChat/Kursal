import { invoke } from '@tauri-apps/api/core';
import type { ContactResponse, OtpResponse } from '$lib/types';

export const generateOtp = (): Promise<OtpResponse> => invoke('generate_otp');

export const publishOtp = (otp: string): Promise<void> => invoke('publish_otp', { otp });

export const fetchOtp = (otp: string): Promise<ContactResponse> => invoke('fetch_otp', { otp });
