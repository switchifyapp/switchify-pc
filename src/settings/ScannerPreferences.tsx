import { Button, Input, Select, MoreOptions } from "../ui/controls";
import { useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { ChevronRight } from 'lucide-react';
import { mouseRepeatStopInstruction, type ScanningController, type ScannerColor, type PointScanConfig } from '../scanning/useScanning';
import { areaOptions, sharedOptions, defaultScanPreferences, usualAfterSelection, savedKeyboardWait, type ScanArea, type ScanOptions } from '../scanning/preferences';
import { SettingGroup, Toggle, OptionGroup } from './controls';

export function ScannerPreferences({ controller }: { controller: ScanningController }) {
  const { config, state, update, chooseKeyboardLayout, pending } = controller;
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
  const names = { point: 'Point scanning', menu: 'Menus', keyboard: 'Keyboard', mouse: 'Mouse scanning' };
  const descriptions = {
    point: 'Choose a place on the screen with a moving line or grid, then choose what to do there.',
    mouse: 'Move the pointer with direction controls and a visible ring. Click, drag, scroll, open Actions for other commands, or open the keyboard from the mouse panel.',
    menu: 'Choose actions after selecting a point.',
    keyboard: 'Choose keys and word suggestions when the scanning keyboard is open.',
  };
  const disabled = !state?.supported;
  const settings = config.scanPreferences ?? defaultScanPreferences;
  const shared = sharedOptions(config);
  const effective = area === 'shared' ? shared : areaOptions(config, area);
  // A scanner shows what it will do, so its usual behaviour stands in for "standard".
  const usual = area === 'shared' ? null : usualAfterSelection(config, area);
  const nextScan = effective.nextScan === 'standard' && usual ? usual.nextScan : effective.nextScan;
  const startFrom = effective.startFrom === 'standard' && usual ? usual.startFrom : effective.startFrom;
  const usualOption = area === 'shared' ? [{ value: 'standard' as const, label: 'Usual for each scanner' }] : [];
  const change = <K extends keyof ScanOptions>(key: K, value: ScanOptions[K] | undefined) => {
    if (area !== 'shared') {
      const overrides = { ...(settings[area] ?? {}), [key]: value };
      update('scanPreferences', { ...settings, [area]: overrides });
    } else if (key === 'automatic') update('automatic', value as boolean);
    else if (key === 'intervalMs') update('blockIntervalMs', value as number);
    else if (key === 'color') update('scannerColor', value as ScannerColor);
    else update('scanPreferences', { ...settings, [key]: value });
  };
  const field = (key: keyof ScanOptions, label: string, children: (locked: boolean) => ReactNode) => {
    const custom = area !== 'shared' && settings[area]?.[key] != null;
    return <div className="scanner-preference" key={key} tabIndex={-1} role="group" aria-label={`${label} setting`}>
      {area !== 'shared' && <div className="scanner-inheritance"><span className={custom ? 'custom' : ''}>{custom ? 'Custom' : 'Default'}</span>
        {custom && <Button type="button" className="text-button" disabled={disabled} aria-label={`Use default for ${label.toLowerCase()}`} onClick={event => { const field = event.currentTarget.closest<HTMLElement>('.scanner-preference'); change(key, undefined); requestAnimationFrame(() => (field?.querySelector<HTMLElement>('input:not(:disabled), select:not(:disabled), button:not(:disabled)') ?? field)?.focus()); }}>Use default</Button>}
      </div>}
      {children(disabled)}
    </div>;
  };
  return <div ref={container} className="scanner-preferences">
    {area === 'shared' ? <header className="scanner-panel-heading"><h2>Two ways to control your PC</h2><p>Select starts the last mode you used: <strong>{config.controlMode === 'mouse' ? 'Mouse scanning' : 'Point scanning'}</strong>. Use Open Point or Open Mouse on a switch to change modes. You can also switch from a scanned panel.</p></header> : <header className="scanner-panel-heading">
      <Button type="button" className="text-button" onClick={() => setArea('shared')}><span aria-hidden="true">←</span> Back to scanning settings</Button>
      <h2 ref={heading} tabIndex={-1}>{names[area]}</h2><p>{descriptions[area]} Change any value to customise it; other settings follow the shared defaults.</p>
    </header>}
    {area === 'shared' && <section className="scanner-areas" aria-label="Scanning modes"><div className="scanner-area-cards">{(['point', 'mouse'] as const).map(key => <Button type="button" className="scanner-area-card" data-area={key} key={key} aria-label={`Customise ${names[key].toLowerCase()}`} onClick={() => openArea(key)}><strong>{names[key]}<ChevronRight size={22} aria-hidden="true" /></strong><span>{descriptions[key]}</span><span className="scanner-area-status" data-current={key === config.controlMode}>{key === config.controlMode ? 'Select starts here' : 'Open with a switch or scanned control'}</span></Button>)}</div></section>}
    {area === 'mouse' && <p className="setting-note">The ring stays visible while Mouse is open. Select a direction to move or a scroll tile to scroll; with Repeat mouse movement and scrolling on, {mouseRepeatStopInstruction(config).toLowerCase()} either action. Actions opens editing, window, browser, media, display and scanning commands, then returns to Mouse. Modified clicks use the current pointer. End a drag before opening Actions. Pointer speed and repeat controls are under Mouse in the sidebar. Switch to Point returns to screen selection.</p>}
    {area === 'point' && <>
      <SettingGroup
        title="Point scan"
        description=""
      >
        <OptionGroup<PointScanConfig["mode"]>
          legend="Method"
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
              <Select
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
              </Select>
            </label>
          </>
        )}

        
      </SettingGroup>
      <MoreOptions label="Click options"><SettingGroup title="Auto selection" description="Automatically left-click the chosen point after a delay. Press a switch again during the delay to open the action menu.">
        <Toggle label="Auto select" checked={config.autoSelectEnabled} disabled={disabled} onChange={(value) => update("autoSelectEnabled", value)} />
        {config.autoSelectEnabled && <label className="exact-speed">
          <span>Auto select delay (seconds)</span>
          <Input type="number" min="0.1" max="100" step="0.1" disabled={disabled} value={config.autoSelectDelayMs / 1000}
            onChange={(event) => { if (event.currentTarget.validity.valid && event.currentTarget.value !== "") update("autoSelectDelayMs", Math.round(event.currentTarget.valueAsNumber * 1000)); }} />
        </label>}
      </SettingGroup></MoreOptions>

    </>}
    {area === 'keyboard' && <>
      <SettingGroup title="Keyboard layout" description="Choose the Letters page layout. Simple QWERTY puts punctuation on its Numbers page; Navigation stays the same.">
        <OptionGroup<PointScanConfig['keyboardLayout']>
          legend="Keyboard layout"
          disabled={disabled || !!pending}
          value={config.keyboardLayout}
          onChange={chooseKeyboardLayout}
          options={[
            { value: 'qwerty', label: 'QWERTY' },
            { value: 'simpleQwerty', label: 'Simple QWERTY' },
            { value: 'commonLetters', label: 'Common letters first' },
          ]}
          note={{ summary: config.keyboardLayout === 'commonLetters'
            ? 'Static English key order: Space E T A O I Backspace; N S H R D L U Enter; C M F W Y P V B; G K J Q X Z Tab Caps lock. Punctuation and modifiers follow.'
            : config.keyboardLayout === 'simpleQwerty'
              ? 'Three familiar letter rows: Q W E R T Y U I O P Backspace; A S D F G H J K L Enter; Shift Z X C V B N M Space. Punctuation is on Numbers.'
              : 'Familiar QWERTY key rows: Q W E R T Y U I O P; A S D F G H J K L; Z X C V B N M. The keyboard footer can switch layouts at any time.' }}
        />
        <p className="setting-note">Simple QWERTY keeps the letter order with fewer keys on its Letters page. Common letters first puts frequent English letters and Space early in row scanning. The keyboard footer cycles through all three layouts; the choice is saved.</p>
      </SettingGroup>
      <p className="setting-note">The scanning keyboard spaces prose punctuation for you. In Simple QWERTY, those marks are on Numbers; the separate numeric period stays a decimal point. Period, question mark, and exclamation mark capitalize the next letter.</p>
    </>}
    {(area === 'keyboard' || area === 'mouse') && <SettingGroup title="Panel position" description="Choose where the keyboard and mouse panels sit from Position on either panel.">
      <Toggle label="Move away from the pointer" checked={config.panelAvoidsPointer} disabled={disabled} onChange={value => update('panelAvoidsPointer', value)} />
      <p className="setting-note">While the pointer is over the panel, it moves to the other side of the screen. It returns when the pointer moves away.</p>
    </SettingGroup>}
    {area === 'keyboard' && <SettingGroup title="Suggestions" description="Show word suggestions while typing with the scanning keyboard.">
      <Toggle label="Word prediction" checked={config.wordPrediction} disabled={disabled} onChange={value => update('wordPrediction', value)} />
      <p className="setting-note">Uses an offline English word database. Suggestions appear as you type and after a space. The keyboard shows when suggestions are loading, unavailable, or have no match. If prediction fails, choose Retry predictions on the keyboard; typing still works. Your typing never leaves this computer and is not used for learning.</p>
    </SettingGroup>}
    <SettingGroup title="Movement" description="">
    {field('automatic', 'Automatic scanning', locked => <Toggle label="Automatic scanning" checked={effective.automatic} disabled={locked} onChange={value => change('automatic', value)} />)}
    {field('intervalMs', 'Auto scan rate', locked => <>
      <div className="exact-speed"><span>Auto scan rate</span><div className="scan-rate-stepper">
        <Button type="button" aria-label="Decrease auto scan interval by 0.1 seconds" disabled={locked || !effective.automatic || effective.intervalMs <= 100} onClick={() => change('intervalMs', Math.max(100, effective.intervalMs - 100))}>−</Button>
        <output aria-label="Auto scan rate" aria-live="polite">{effective.intervalMs / 1000} s</output>
        <Button type="button" aria-label="Increase auto scan interval by 0.1 seconds" disabled={locked || !effective.automatic || effective.intervalMs >= 10000} onClick={() => change('intervalMs', Math.min(10000, effective.intervalMs + 100))}>+</Button>
      </div></div>
      <p className="setting-note">{effective.automatic ? 'Seconds per highlight. Less time means faster scanning.' : 'Used in automatic scanning. Your rate is kept while scanning manually.'}</p>
    </>)}
    </SettingGroup>
    <MoreOptions>
    {field('nextScan', 'After a selection', locked => <OptionGroup<ScanOptions['nextScan']> legend="After a selection" columns={area === 'shared' ? 'three' : undefined} disabled={locked || (area !== 'shared' && area !== 'point' && !effective.automatic)} value={nextScan}
      options={[...usualOption, { value: 'automatic', label: 'Keep scanning' }, { value: 'wait', label: 'Wait for Select' }]} onChange={value => change('nextScan', value)}
      note={{ summary: area === 'point' ? 'After a click, the next point scan starts by itself or waits for Select. Scanning still stops at the pass limit.'
        : area !== 'shared' && !effective.automatic ? 'Used in automatic scanning. Your choice is kept while scanning manually.'
        : area === 'shared' ? 'After a selection that does something, such as typing a key or clicking, scanning starts again by itself or waits for Select. Usually, scanning waits after a click and starts again after anything else.' + (savedKeyboardWait(config) ? ' The keyboard waits, as saved earlier.' : '') + ' Waiting applies to scanners that scan automatically.'
        : 'After a selection that does something, such as typing a key or scrolling, scanning starts again by itself or waits for Select.' + (area === 'keyboard' && savedKeyboardWait(config) ? ' Waiting is the default here because it was saved earlier as After typing.' : '') }} />)}
    {area !== 'point' && field('startFrom', 'Start again from', locked => <OptionGroup<ScanOptions['startFrom']> legend="Start again from" columns={area === 'shared' ? 'three' : undefined} disabled={locked} value={startFrom}
      options={[...usualOption, { value: 'beginning', label: 'The beginning' }, { value: 'selection', label: 'Where I selected' }]} onChange={value => change('startFrom', value)}
      note={{ summary: area === 'shared' ? 'Where scanning starts after a selection that does something. Usually, menus stay where you selected, and the keyboard and mouse panel return to the beginning. Point scanning always starts from the beginning.'
        : area === 'keyboard' ? 'Where scanning starts after typing a key. After a suggestion, or when changing page, it starts from the beginning.'
        : 'Where scanning starts after a selection that does something. Opening another page or menu starts from its beginning.' }} />)}
    {field('direction', 'Initial direction', locked => <OptionGroup<ScanOptions['direction']> legend="Initial direction" disabled={locked} value={effective.direction} options={[{ value: 'forward', label: 'Forward' }, { value: 'reverse', label: 'Reverse' }]} onChange={value => change('direction', value)} />)}
    {field('passLimit', 'Pass limit', locked => <OptionGroup<number> legend="Pass limit" disabled={locked} value={effective.passLimit} options={[1, 2, 3, 5, 0].map(value => ({ value, label: value ? `${value} ${value === 1 ? 'pass' : 'passes'}` : 'Unlimited' }))} onChange={value => change('passLimit', value)}
      note={{ summary: 'After this many automatic passes, scanning waits for Select. Unlimited keeps scanning until you pause or stop.' }} />)}
    {area !== 'point' && field('pattern', 'Item scanning', locked => <OptionGroup<ScanOptions['pattern']> legend="Item scanning" disabled={locked} value={effective.pattern} options={[{ value: 'grouped', label: 'Groups, then items' }, { value: 'linear', label: 'One item at a time' }]} onChange={value => change('pattern', value)}
      note={{ summary: 'Grouped scanning chooses a row or group before an item. Linear scanning visits each item directly. Point scanning uses its own line and grid modes.' }} />)}

    <SettingGroup title="Appearance" description="Choose a highlight that is easy to see.">
    {field('color', 'Scanner colour', locked => <fieldset disabled={locked}><legend>Scanner colour</legend><div className="scanner-colours">
      {(['red', 'green', 'blue', 'yellow', 'white'] as const).map(colour => <label key={colour}><Input type="radio" name="scanner-colour" value={colour} checked={effective.color === colour} onChange={() => change('color', colour)} /><span className={`color-swatch ${colour}`} aria-hidden="true" /><span>{colour[0].toUpperCase() + colour.slice(1)}</span></label>)}
    </div></fieldset>)}
    {field('thickness', 'Highlight thickness', locked => <OptionGroup<ScanOptions['thickness']> legend="Highlight thickness" disabled={locked} value={effective.thickness} options={(['thin', 'standard', 'thick'] as const).map(value => ({ value, label: value[0].toUpperCase() + value.slice(1) }))} onChange={value => change('thickness', value)} />)}
    <div className={`scanner-sample ${effective.color}`} role="img" aria-label={`${effective.color} scanner highlight sample`}>
      <span className={`scanner-sample-selection ${effective.thickness}`}>Selected area</span>
    </div>
    </SettingGroup>
    {area === 'shared' && <section className="scanner-areas" aria-labelledby="scanner-areas-title">
      <header><h2 id="scanner-areas-title">Advanced panel settings</h2><p>Menus and Keyboard can use different scan settings from the shared defaults.</p></header>
      <div className="scanner-area-cards">{(['menu', 'keyboard'] as const).map(key => {
        const options = areaOptions(config, key);
        const count = Object.values(settings[key] ?? {}).filter(value => value != null).length;
        return <Button type="button" className="scanner-area-card" data-area={key} key={key} aria-label={`Customise ${names[key].toLowerCase()}`} onClick={() => openArea(key)}>
          <strong>{names[key]}<ChevronRight size={22} aria-hidden="true" /></strong>
          <span>{options.automatic ? `Automatic · ${options.intervalMs / 1000}s` : 'Manual'} · {options.direction === 'forward' ? 'Forward' : 'Reverse'}</span>
          <span>{options.pattern === 'grouped' ? 'Groups, then items' : 'One item at a time'}</span>
          <span className="scanner-area-appearance"><i className={`color-swatch ${options.color}`} aria-hidden="true" />{options.color} · {options.thickness}</span>
          <span className="scanner-area-badge">{count ? `${count} custom ${count === 1 ? 'setting' : 'settings'}` : 'Using defaults'}</span>
        </Button>;
      })}</div>
    </section>}
    </MoreOptions>
    {area !== 'shared' && <Button type="button" className="secondary" disabled={disabled || Object.values(settings[area] ?? {}).every(value => value == null)} onClick={() => update('scanPreferences', { ...settings, [area]: {} })}>Reset scanning overrides</Button>}
    <p className="setting-note scanner-save-note">Press Select to scan again.</p>
  </div>;
}
