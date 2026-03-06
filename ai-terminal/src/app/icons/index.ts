import { aiPanelVisible } from './ai-panel-visible.icon';
import { aiPanelHidden } from './ai-panel-hidden.icon';
import { copy } from './copy.icon';
import { copyToTerminal } from './copy-to-terminal.icon';
import { execute } from './execute.icon';

export type IconName =
  | 'ai-panel-visible'
  | 'ai-panel-hidden'
  | 'copy'
  | 'copy-to-terminal'
  | 'execute';

const ICONS: Record<IconName, string> = {
  'ai-panel-visible': aiPanelVisible,
  'ai-panel-hidden': aiPanelHidden,
  'copy': copy,
  'copy-to-terminal': copyToTerminal,
  'execute': execute,
};

const SVG_ATTRS = 'viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"';

/**
 * Returns the full SVG markup for an icon by name.
 * Wraps the icon content with standard SVG attributes.
 */
export function getIcon(name: IconName, size: number = 24): string {
  const content = ICONS[name];
  if (!content) {
    console.warn(`Icon not found: ${name}`);
    return '';
  }
  return `<svg width="${size}" height="${size}" ${SVG_ATTRS}>${content}</svg>`;
}
