export type TimeFormat = '24h' | '12h';

let hour12Pref = false;

export function setTimeFormatPref(value: TimeFormat) {
  hour12Pref = value === '12h';
}

export function clockOptions(): Intl.DateTimeFormatOptions {
  return {
    hour: '2-digit',
    minute: '2-digit',
    hour12: hour12Pref,
  };
}
