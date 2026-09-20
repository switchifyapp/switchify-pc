import { useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import type { ScanningController, ScannerColor, PointScanConfig } from '../scanning/useScanning';
import { areaOptions, sharedOptions, defaultScanPreferences, scanIntervals, type ScanArea, type ScanOptions } from '../scanning/preferences';
import { SettingGroup, Toggle, OptionGroup } from './controls';

export function ScannerPreferences({ controller }: { controller: ScanningController }) {
  const { config, state, update } = controller;
  const [area, setArea] = useState<'shared' | ScanArea>('shared');
  const heading = useRef<HTMLHeadingElement>(null);
  const container = useRef<HTMLDivElement>(null);
  const returnArea = useRef<ScanArea | null>(null);
  const overviewScroll = useRef(0);
  const changedView = useRef(false);
  const scrollContainer = () => {
    const main = container.current?.closest('main');
    return main && main.scrollHeight > main.clientHeight ? main : document.scrollingElement ?? document.documentElement;
  };
  useLayoutEffect(() => {
    if (!changedView.current) return;
    const scroller = scrollContainer();
    if (area === 'shared') {
      container.current?.querySelector<HTMLButtonElement>(`[data-area="${returnArea.current}"]`)?.focus({ preventScroll: true });
      if (scroller) scroller.scrollTop = overviewScroll.current;
    } else {
      heading.current?.focus({ preventScroll: true });
      heading.current?.closest('header')?.scrollIntoView?.({ block: 'start', behavior: 'instant' });
    }
  }, [area]);
  const openArea = (next: ScanArea) => {
    overviewScroll.current = scrollContainer().scrollTop;
    returnArea.current = next;
    changedView.current = true;
    setArea(next);
  };
  const names = { point: 'Point scanning', menu: 'Menus', keyboard: 'Keyboard' };
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
    const custom = area !== 'shared' && settings[area][key] != null;
    return <div className="scanner-preference" key={key} tabIndex={-1} role="group" aria-label={`${label} setting`}>
      {area !== 'shared' && <div className="scanner-inheritance"><span className={custom ? 'custom' : ''}>{custom ? 'Custom' : 'Default'}</span>
        {custom && <button type="button" className="text-button" disabled={disabled} aria-label={`Use default for ${label.toLowerCase()}`} onClick={event => { const field = event.currentTarget.closest<HTMLElement>('.scanner-preference'); change(key, undefined); requestAnimationFrame(() => (field?.querySelector<HTMLElement>('input:not(:disabled), select:not(:disabled), button:not(:disabled)') ?? field)?.focus()); }}>Use default</button>}
      </div>}
      {children(disabled)}
    </div>;
  };
  return <div ref={container} className="scanner-preferences">
    {area === 'shared' && <section className="scanner-areas" aria-labelledby="scanner-areas-title">
      <header><h2 id="scanner-areas-title">Customise an area</h2><p>Give an area different settings, or keep using your defaults.</p></header>
      <div className="scanner-area-cards">{(['point', 'menu', 'keyboard'] as const).map(key => {
        const options = areaOptions(config, key);
        const count = Object.entries(settings[key]).filter(([name, value]) => value != null && !(key === 'point' && name === 'pattern')).length;
        return <button type="button" className="scanner-area-card" data-area={key} key={key} aria-label={`Customise ${names[key].toLowerCase()}`} onClick={() => openArea(key)}>
          <strong>{names[key]} <span aria-hidden="true">→</span></strong>
          <span>{options.automatic ? `Automatic · ${options.intervalMs / 1000}s` : 'Manual'} · {options.direction === 'forward' ? 'Forward' : 'Reverse'}</span>
          <span>{key === 'point' ? (config.mode === 'grid' ? 'Grid then line' : 'Line only') : (options.pattern === 'grouped' ? 'Groups, then items' : 'One item at a time')}</span>
          <span className="scanner-area-appearance"><i className={`color-swatch ${options.color}`} aria-hidden="true" />{options.color} · {options.thickness}</span>
          <span className="scanner-area-badge">{count ? `${count} custom ${count === 1 ? 'setting' : 'settings'}` : 'Using defaults'}</span>
        </button>;
      })}</div>
    </section>}
    {area === 'shared' ? <header className="scanner-panel-heading"><h2>Shared defaults</h2><p>Start here. Each scanning area follows these settings unless you customise it.</p></header> : <header className="scanner-panel-heading">
      <button type="button" className="text-button" onClick={() => setArea('shared')}><span aria-hidden="true">←</span> Back to scanning settings</button>
      <h2 ref={heading} tabIndex={-1}>{names[area]}</h2><p>Change any value to customise it. Other settings keep following your defaults.</p>
    </header>}
    {area === 'point' && <>
      <SettingGroup
        title="Point scan"
        description="Choose how the scanning lines find a point. Changes apply straight away."
      >
        <OptionGroup<PointScanConfig["mode"]>
          legend="Mode"
          disabled={disabled}
          value={config.mode}
          onChange={(value) => update("mode", value)}
          options={[
            { value: "line", label: "Line only" },
            { value: "grid", label: "Grid then line" },
          ]}
        />

        <OptionGroup<number>
          legend="Line speed"
          columns="five"
          disabled={disabled}
          value={config.speed}
          onChange={(value) => update("speed", value)}
          options={["Very slow", "Slow", "Medium", "Fast", "Very fast"].map(
            (label, value) => ({ label, value }),
          )}
        />

        {config.mode === "grid" && (
          <>
            <label className="exact-speed">
              <span>Grid size</span>
              <select
                disabled={disabled}
                value={config.gridSize}
                onChange={(event) =>
                  update("gridSize", Number(event.target.value))
                }
              >
                {Array.from({ length: 9 }, (_, i) => i + 2).map((n) => (
                  <option key={n} value={n}>
                    {n} × {n}
                  </option>
                ))}
              </select>
            </label>
          </>
        )}

        
      </SettingGroup>
      <SettingGroup title="Auto selection" description="Automatically left-click the chosen point after a delay. Press a switch again during the delay to open the action menu.">
        <Toggle label="Auto select" checked={config.autoSelectEnabled} disabled={disabled} onChange={(value) => update("autoSelectEnabled", value)} />
        {config.autoSelectEnabled && <label className="exact-speed">
          <span>Auto select delay (seconds)</span>
          <input type="number" min="0.1" max="100" step="0.1" disabled={disabled} value={config.autoSelectDelayMs / 1000}
            onChange={(event) => { if (event.currentTarget.validity.valid && event.currentTarget.value !== "") update("autoSelectDelayMs", Math.round(event.currentTarget.valueAsNumber * 1000)); }} />
        </label>}
      </SettingGroup>

    </>}
    {area === 'keyboard' && <SettingGroup title="Suggestions" description="Show word suggestions while typing with the scanning keyboard.">
      <Toggle label="Word prediction" checked={config.wordPrediction} disabled={disabled} onChange={value => update('wordPrediction', value)} />
    </SettingGroup>}
    <SettingGroup title="Movement" description="Choose how scanning advances and when it waits for you.">
    {field('automatic', 'Automatic scanning', locked => <Toggle label="Automatic scanning" checked={effective.automatic} disabled={locked} onChange={value => change('automatic', value)} />)}
    {area === 'keyboard' && <OptionGroup<'continue' | 'wait'> legend="After typing" disabled={disabled || !effective.automatic} value={config.keyboardWaitAfterTyping ? 'wait' : 'continue'}
      options={[{ value: 'continue', label: 'Continue scanning' }, { value: 'wait', label: 'Wait for Select' }]}
      onChange={value => update('keyboardWaitAfterTyping', value === 'wait')}
      note={{ summary: effective.automatic ? 'After typing a key or suggestion, wait for Select before scanning again.' : 'Used in automatic keyboard scanning. Your choice is kept while scanning manually.' }} />}
    {field('intervalMs', 'Auto scan rate', locked => <>
      <label className="exact-speed"><span>Auto scan rate</span><select disabled={locked || !effective.automatic} value={effective.intervalMs} onChange={event => change('intervalMs', Number(event.target.value))}>
        {[...new Set([...scanIntervals, effective.intervalMs])].sort((a, b) => a - b).map(value => <option key={value} value={value}>{value / 1000} {value === 1000 ? 'second' : 'seconds'}</option>)}
      </select></label>
      <p className="setting-note">{effective.automatic ? 'Time each row or item stays highlighted. Lower values scan faster. Line speed is separate.' : 'Used in automatic scanning. Your rate is kept while scanning manually.'}</p>
    </>)}
    {field('direction', 'Initial direction', locked => <OptionGroup<ScanOptions['direction']> legend="Initial direction" disabled={locked} value={effective.direction} options={[{ value: 'forward', label: 'Forward' }, { value: 'reverse', label: 'Reverse' }]} onChange={value => change('direction', value)} />)}
    {field('passLimit', 'Pass limit', locked => <OptionGroup<number> legend="Pass limit" disabled={locked} value={effective.passLimit} options={[1, 2, 3, 5, 0].map(value => ({ value, label: value ? `${value} ${value === 1 ? 'pass' : 'passes'}` : 'Unlimited' }))} onChange={value => change('passLimit', value)}
      note={{ summary: 'After this many automatic passes, scanning waits for Select. Unlimited keeps scanning until you pause or stop.' }} />)}
    {area !== 'point' && field('pattern', 'Item scanning', locked => <OptionGroup<ScanOptions['pattern']> legend="Item scanning" disabled={locked} value={effective.pattern} options={[{ value: 'grouped', label: 'Groups, then items' }, { value: 'linear', label: 'One item at a time' }]} onChange={value => change('pattern', value)}
      note={{ summary: 'Grouped scanning chooses a row or group before an item. Linear scanning visits each item directly. Point scanning uses its own line and grid modes.' }} />)}
    </SettingGroup>
    <SettingGroup title="Appearance" description="Choose a highlight that is easy to see.">
    {field('color', 'Scanner colour', locked => <fieldset disabled={locked}><legend>Scanner colour</legend><div className="scanner-colours">
      {(['red', 'green', 'blue', 'yellow', 'white'] as const).map(colour => <label key={colour}><input type="radio" name="scanner-colour" value={colour} checked={effective.color === colour} onChange={() => change('color', colour)} /><span className={`color-swatch ${colour}`} aria-hidden="true" /><span>{colour[0].toUpperCase() + colour.slice(1)}</span></label>)}
    </div></fieldset>)}
    {field('thickness', 'Highlight thickness', locked => <OptionGroup<ScanOptions['thickness']> legend="Highlight thickness" disabled={locked} value={effective.thickness} options={(['thin', 'standard', 'thick'] as const).map(value => ({ value, label: value[0].toUpperCase() + value.slice(1) }))} onChange={value => change('thickness', value)} />)}
    <div className={`scanner-sample ${effective.color}`} role="img" aria-label={`${effective.color} scanner highlight sample`}>
      <span className={`scanner-sample-selection ${effective.thickness}`}>Selected area</span>
    </div>
    </SettingGroup>
    {area !== 'shared' && <button type="button" className="secondary" disabled={disabled || Object.values(settings[area]).every(value => value == null)} onClick={() => update('scanPreferences', { ...settings, [area]: {} })}>Reset scanning overrides</button>}
    <p className="setting-note scanner-save-note">Changes save automatically, cancel the current scan and wait for Select.</p>
  </div>;
}
