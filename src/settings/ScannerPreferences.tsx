import { useState, type ReactNode } from 'react';
import type { ScanningController, ScannerColor } from '../scanning/useScanning';
import { areaOptions, sharedOptions, defaultScanPreferences, scanIntervals, type ScanArea, type ScanOptions } from '../scanning/preferences';
import { SettingGroup, Toggle, OptionGroup, secondsOptions } from './controls';

export function ScannerPreferences({ controller }: { controller: ScanningController }) {
  const { config, state, update } = controller;
  const [area, setArea] = useState<'shared' | ScanArea>('shared');
  const disabled = !state?.supported;
  const settings = config.scanPreferences ?? defaultScanPreferences;
  const shared = sharedOptions(config);
  const effective = area === 'shared' ? shared : areaOptions(config, area);
  const change = <K extends keyof ScanOptions>(key: K, value: ScanOptions[K] | undefined) => {
    if (area !== 'shared') {
      const overrides = { ...settings[area], [key]: value };
      update('scanPreferences', { ...settings, [area]: overrides });
    } else if (key === 'automatic') update('automatic', value as boolean);
    else if (key === 'intervalMs') update('blockIntervalMs', value as number);
    else if (key === 'color') update('scannerColor', value as ScannerColor);
    else update('scanPreferences', { ...settings, [key]: value });
  };
  const field = (key: keyof ScanOptions, label: string, children: (locked: boolean) => ReactNode) => {
    const inherited = area !== 'shared' && settings[area][key] == null;
    return <div className="scanner-preference" key={key}>
      {area !== 'shared' && <Toggle label={`Use shared ${label.toLowerCase()}`} checked={inherited} disabled={disabled}
        onChange={(inherit) => change(key, inherit ? undefined : effective[key])} />}
      {children(disabled || inherited)}
    </div>;
  };
  return <SettingGroup title="Scanning defaults and overrides" description="Set shared defaults, then customise individual areas when you need different behaviour.">
    <label className="exact-speed"><span>Customise scanning for</span><select value={area} onChange={event => setArea(event.target.value as typeof area)}>
      <option value="shared">Shared defaults</option><option value="point">Point scanning</option><option value="menu">Menus</option><option value="keyboard">Keyboard</option><option value="app">App screens</option>
    </select></label>
    {area !== 'shared' && <p className="setting-note">Checked “Use shared” controls follow your shared defaults. Uncheck one to choose a value for this area.</p>}
    {field('automatic', 'Automatic scanning', locked => <Toggle label="Automatic scanning" checked={effective.automatic} disabled={locked} onChange={value => change('automatic', value)} />)}
    {field('intervalMs', 'Auto scan rate', locked => <OptionGroup<number> legend="Auto scan rate" disabled={locked} value={effective.intervalMs} options={secondsOptions(scanIntervals)} onChange={value => change('intervalMs', value)}
      note={{ summary: 'How long each grid row, grid cell, or action-menu item stays highlighted before scanning moves on automatically. Shorter times scan faster; longer times give you more time to select. Line speed controls how fast the scanning lines move.' }} />)}
    {field('direction', 'Initial direction', locked => <OptionGroup<ScanOptions['direction']> legend="Initial direction" disabled={locked} value={effective.direction} options={[{ value: 'forward', label: 'Forward' }, { value: 'reverse', label: 'Reverse' }]} onChange={value => change('direction', value)} />)}
    {field('passLimit', 'Pass limit', locked => <OptionGroup<number> legend="Pass limit" disabled={locked} value={effective.passLimit} options={[1, 2, 3, 5, 0].map(value => ({ value, label: value ? `${value} ${value === 1 ? 'pass' : 'passes'}` : 'Unlimited' }))} onChange={value => change('passLimit', value)}
      note={{ summary: 'After this many automatic passes, scanning waits for Select. Unlimited keeps scanning until you pause or stop.' }} />)}
    {area !== 'point' && field('pattern', 'Item scanning', locked => <OptionGroup<ScanOptions['pattern']> legend="Item scanning" disabled={locked} value={effective.pattern} options={[{ value: 'grouped', label: 'Groups, then items' }, { value: 'linear', label: 'One item at a time' }]} onChange={value => change('pattern', value)}
      note={{ summary: 'Grouped scanning chooses a row or group before an item. Linear scanning visits each item directly. Point scanning uses its own line and grid modes.' }} />)}
    {field('color', 'Scanner colour', locked => <fieldset disabled={locked}><legend>Scanner colour</legend><div className="scanner-colours">
      {(['red', 'green', 'blue', 'yellow', 'white'] as const).map(colour => <label key={colour}><input type="radio" name="scanner-colour" value={colour} checked={effective.color === colour} onChange={() => change('color', colour)} /><span className={`color-swatch ${colour}`} aria-hidden="true" /><span>{colour[0].toUpperCase() + colour.slice(1)}</span></label>)}
    </div></fieldset>)}
    {field('thickness', 'Highlight thickness', locked => <OptionGroup<ScanOptions['thickness']> legend="Highlight thickness" disabled={locked} value={effective.thickness} options={(['thin', 'standard', 'thick'] as const).map(value => ({ value, label: value[0].toUpperCase() + value.slice(1) }))} onChange={value => change('thickness', value)} />)}
    <div className={`scanner-sample ${effective.color}`} role="img" aria-label={`${effective.color} scanner highlight sample`}>
      <span className={`scanner-sample-selection ${effective.thickness}`}>Selected area</span>
    </div>
    {area !== 'shared' && <button type="button" className="secondary" disabled={disabled || Object.values(settings[area]).every(value => value == null)} onClick={() => update('scanPreferences', { ...settings, [area]: {} })}>Use shared settings for this area</button>}
    <p>Assign switch actions in the Switches page. All actions run on release. Holding a switch freezes movement. Manual scanning needs Select, Next and Previous. After clicking, or reaching the pass limit, use Select to start again.</p>
    <p className="setting-note">Saving changes cancels the current scan and waits for Select.</p>
  </SettingGroup>;
}
