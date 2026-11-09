/** Runtime flags: `?mock=1` swaps in fake backend/player so the UI can be screenshotted without the desktop app. */
const params = new URLSearchParams(typeof location === 'undefined' ? '' : location.search);

export const isMock = params.has('mock');
export const mockParams = params;
