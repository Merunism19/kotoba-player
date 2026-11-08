/** Inline SVG icons (stroke style, 24px grid) used across the player controls. */
import type { ReactNode } from 'react';

function Svg({ children, fill = false, size = 18 }: { children: ReactNode; fill?: boolean; size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill={fill ? 'currentColor' : 'none'}
      stroke={fill ? 'none' : 'currentColor'}
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {children}
    </svg>
  );
}

export const IconBack = () => <Svg><path d="M15 18l-6-6 6-6" /></Svg>;
export const IconPlay = () => <Svg fill size={20}><path d="M7 4.5v15l12.5-7.5z" /></Svg>;
export const IconPause = () => <Svg fill size={20}><path d="M6.5 4.5h4v15h-4zM13.5 4.5h4v15h-4z" /></Svg>;
export const IconPrev = () => <Svg fill><path d="M6 5h2v14H6zM20 5v14L9 12z" /></Svg>;
export const IconNext = () => <Svg fill><path d="M16 5h2v14h-2zM4 5v14l11-7z" /></Svg>;
export const IconReplay = () => <Svg><path d="M3 12a9 9 0 109-9 9 9 0 00-6.4 2.6L3 8" /><path d="M3 3v5h5" /></Svg>;
export const IconVolume = () => <Svg><path d="M11 5L6 9H3v6h3l5 4V5z" /><path d="M15.5 8.5a5 5 0 010 7M18.5 5.5a9 9 0 010 13" /></Svg>;
export const IconMute = () => <Svg><path d="M11 5L6 9H3v6h3l5 4V5z" /><path d="M22 9l-6 6M16 9l6 6" /></Svg>;
export const IconFullscreen = () => <Svg><path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" /></Svg>;
export const IconExitFullscreen = () => <Svg><path d="M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5" /></Svg>;
export const IconHeadphones = () => <Svg size={15}><path d="M4 15v-3a8 8 0 0116 0v3" /><rect x="3" y="14" width="4" height="6" rx="1.5" /><rect x="17" y="14" width="4" height="6" rx="1.5" /></Svg>;
export const IconSubs = () => <Svg size={15}><rect x="3" y="5" width="18" height="14" rx="3" /><path d="M7 11h4M13 11h4M7 15h2M12 15h5" /></Svg>;
export const IconCheck = () => <Svg size={16}><path d="M5 12.5l4.5 4.5L19 7.5" /></Svg>;
export const IconClock = () => <Svg size={14}><circle cx="12" cy="12" r="9" /><path d="M12 7v5l3 2" /></Svg>;
export const IconUpload = () => <Svg size={30}><path d="M12 15V4M7 9l5-5 5 5" /><path d="M20 15v4a2 2 0 01-2 2H6a2 2 0 01-2-2v-4" /></Svg>;
export const IconAlert = () => <Svg size={18}><circle cx="12" cy="12" r="9" /><path d="M12 7.5v5M12 16h.01" /></Svg>;
export const IconClose = () => <Svg size={16}><path d="M6 6l12 12M18 6L6 18" /></Svg>;
export const IconSpeaker = () => <Svg size={19}><path d="M11 5L6 9H3v6h3l5 4V5z" /><path d="M15.5 8.5a5 5 0 010 7M18.5 5.5a9 9 0 010 13" /></Svg>;
export const IconPlus = () => <Svg size={16}><path d="M12 5v14M5 12h14" /></Svg>;
export const IconGear = () => <Svg size={19}><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.7 1.7 0 00.3 1.8l.1.1a2 2 0 11-2.8 2.8l-.1-.1a1.7 1.7 0 00-1.8-.3 1.7 1.7 0 00-1 1.5V21a2 2 0 11-4 0v-.1a1.7 1.7 0 00-1.1-1.5 1.7 1.7 0 00-1.8.3l-.1.1a2 2 0 11-2.8-2.8l.1-.1a1.7 1.7 0 00.3-1.8 1.7 1.7 0 00-1.5-1H3a2 2 0 110-4h.1a1.7 1.7 0 001.5-1.1 1.7 1.7 0 00-.3-1.8l-.1-.1a2 2 0 112.8-2.8l.1.1a1.7 1.7 0 001.8.3H9a1.7 1.7 0 001-1.5V3a2 2 0 114 0v.1a1.7 1.7 0 001 1.5 1.7 1.7 0 001.8-.3l.1-.1a2 2 0 112.8 2.8l-.1.1a1.7 1.7 0 00-.3 1.8V9a1.7 1.7 0 001.5 1H21a2 2 0 110 4h-.1a1.7 1.7 0 00-1.5 1z" /></Svg>;
export const IconList = () => <Svg size={19}><path d="M4 6h16M4 12h10M4 18h13" /></Svg>;
export const IconSearch = () => <Svg size={15}><circle cx="11" cy="11" r="7" /><path d="M20 20l-3.5-3.5" /></Svg>;
export const IconSync = () => <Svg size={15}><path d="M4 12a8 8 0 0113.7-5.6L20 8.5M20 4v4.5h-4.5M20 12a8 8 0 01-13.7 5.6L4 15.5M4 20v-4.5h4.5" /></Svg>;
export const IconChevron = ({ dir }: { dir: 'left' | 'right' | 'up' | 'down' }) => (
  <Svg size={14}>
    <path d={{ left: 'M15 18l-6-6 6-6', right: 'M9 18l6-6-6-6', up: 'M18 15l-6-6-6 6', down: 'M6 9l6 6 6-6' }[dir]} />
  </Svg>
);
