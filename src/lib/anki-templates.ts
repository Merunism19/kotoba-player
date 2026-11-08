/** The Kotoba Mining templates and CSS from `anki/`, bundled as strings (Vite `?raw`) so the mining preview renders the real files. */
import front from '../../anki/front.html?raw';
import back from '../../anki/back.html?raw';
import css from '../../anki/style.css?raw';

export const TEMPLATES = { front, back } as const;
export const CARD_CSS: string = css;
