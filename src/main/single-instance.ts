import { shouldStartHidden } from './system-startup';

export type SecondInstanceAction = 'showMainWindow' | 'ignore';

export function secondInstanceAction(argv: string[], platform: NodeJS.Platform): SecondInstanceAction {
  return shouldStartHidden(argv, platform) ? 'ignore' : 'showMainWindow';
}
