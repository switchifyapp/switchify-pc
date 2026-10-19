import { Button, Input, Select, MoreOptions } from "../ui/controls";
import { useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { ChevronRight } from 'lucide-react';
import { lastModeName, mouseRepeatStopInstruction, selectOpensHome, type ScanningController, type ScannerColor, type PointScanConfig } from '../scanning/useScanning';
import { areaOptions, sharedOptions, defaultScanPreferences, usualAfterSelection, savedKeyboardWait, itemScanning, itemScanningLabels, type ItemScanning, type PanelSize, type ScanArea, type ScanOptions } from '../scanning/preferences';
import { SettingGroup, Toggle, OptionGroup } from './controls';
import { api } from '../api';
import type { AppSettings } from '../types';

const speedOptions = ["Very slow", "Slow", "Medium", "Fast", "Very fast"].map((label, value) => ({ label, value }));

export function ScannerPreferences({ controller, settings: appSettings, onSettingsChange, platform }: { controller: ScanningController; settings?: AppSettings; onSettingsChange?: (next: AppSettings) => void; platform?: string }) {
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
    menu: 'Choose what to control in Home, and actions after selecting a point. Menus sit where the keyboard and mouse panels do.',
    keyboard: 'Choose keys and word suggestions when the scanning keyboard is open.',
  };
  const disabled = !state?.supported;
  const opensHome = selectOpensHome(config);
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
      // Row groups belong to the area's item scanning choice.
      const overrides = { ...(settings[area] ?? {}), [key]: value, ...(key === 'pattern' && value == null ? { rowGroups: undefined } : {}) };
      update('scanPreferences', { ...settings, [area]: overrides });
    } else if (key === 'automatic') update('automatic', value as boolean);
    else if (key === 'intervalMs') update('blockIntervalMs', value as number);
    else if (key === 'color') update('scannerColor', value as ScannerColor);
    else update('scanPreferences', { ...settings, [key]: value });
  };
  const chooseItemScanning = (choice: ItemScanning) => {
    const chosen = { pattern: choice === 'linear' ? 'linear' as const : 'grouped' as const, rowGroups: choice === 'rowGroups' };
    update('scanPreferences', area === 'shared' ? { ...settings, ...chosen } : { ...settings, [area]: { ...(settings[area] ?? {}), ...chosen } });
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
    {area === 'shared' ? <header className="scanner-panel-heading"><h2>Two ways to control your PC</h2><p>{opensHome
      ? <>Select opens <strong>Home</strong>. After an action, it carries on in the last mode you used: <strong>{lastModeName(config)}</strong>.</>
      : <>Select starts the last mode you used: <strong>{lastModeName(config)}</strong>.</>} Use Open Point or Open Mouse on a switch to change modes. You can also switch from a scanned panel.</p></header> : <header className="scanner-panel-heading">
      <Button type="button" className="text-button" onClick={() => setArea('shared')}><span aria-hidden="true">←</span> Back to scanning settings</Button>
      <h2 ref={heading} tabIndex={-1}>{names[area]}</h2><p>{descriptions[area]} Change any value to customise it; other settings follow the shared defaults.</p>
    </header>}
    {area === 'shared' && <section className="scanner-areas" aria-label="Scanning modes"><div className="scanner-area-cards">{(['point', 'mouse'] as const).map(key => <Button type="button" className="scanner-area-card" data-area={key} key={key} aria-label={`Customise ${names[key].toLowerCase()}`} onClick={() => openArea(key)}><strong>{names[key]}<ChevronRight size={22} aria-hidden="true" /></strong><span>{descriptions[key]}</span><span className="scanner-area-status" data-current={key === config.controlMode}>{key !== config.controlMode ? 'Open with a switch or scanned control' : opensHome ? 'Used last' : 'Select starts here'}</span></Button>)}</div></section>}
    {area === 'shared' && <SettingGroup title="Starting" description="">
      <OptionGroup<NonNullable<PointScanConfig['startWith']>>
        legend="Select starts"
        disabled={disabled}
        value={config.startWith ?? 'home'}
        onChange={value => update('startWith', value)}
        options={[{ value: 'home', label: 'Home menu' }, { value: 'lastMode', label: 'Last mode used' }]}
        note={{ summary: opensHome
          ? 'Home offers Point, Mouse and Keyboard, plus Apps and windows to bring another app forward, without focusing it first. After a click or other action, Select carries on in the same mode. Close menu or Stop scanning returns to Home.'
          : 'Select starts the last mode you used straight away. Open Home from the Point action menu or Mouse Actions.' }}
      />
    </SettingGroup>}
    {area === 'mouse' && <p className="setting-note">The ring stays visible while Mouse is open. Select a direction to move or a scroll tile to scroll; with Repeat mouse movement and scrolling on, {mouseRepeatStopInstruction(config).toLowerCase()} either action. Actions opens editing, window, browser, media, display and scanning commands, then returns to Mouse. Modified clicks use the current pointer. End a drag before opening Actions. Pointer speed and repeat controls are under Mouse in the sidebar. Switch to Point returns to screen selection.</p>}
    {area === 'mouse' && <SettingGroup title="Radar" description="Radar picks an area, then a direction, then a distance to move the pointer.">
      <OptionGroup<number>
        legend="Radar speed"
        columns="five"
        disabled={disabled}
        value={config.radarSpeed ?? config.speed}
        onChange={value => update('radarSpeed', value)}
        options={speedOptions}
        note={{ summary: 'How fast the beam turns and grows. Areas move at the auto scan rate.' + (config.radarSpeed == null ? ' Follows Line speed in Point scanning until you choose a speed here.' : '') }}
      />
      {config.radarSpeed != null && <Button type="button" className="text-button" disabled={disabled} onClick={event => {
        const group = event.currentTarget.closest<HTMLElement>('.setting-group');
        update('radarSpeed', null);
        // The button goes once the radar follows line speed, so keep focus on the speeds.
        requestAnimationFrame(() => group?.querySelector<HTMLElement>('[aria-pressed="true"]')?.focus());
      }}>Follow line speed</Button>}
    </SettingGroup>}
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
          options={speedOptions}
          note={config.radarSpeed == null ? { summary: "Radar in Mouse scanning follows this speed until it has its own." } : undefined}
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
      <SettingGroup title="Keyboard layout" description="Choose the Letters page layout. Navigation, Numbers and Symbols stay the same in every layout.">
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
            ? 'Static English key order: Space E T A O I Backspace; N S H R D L U Enter; C M F W Y P V B; G K J Q X Z Tab Caps lock; comma, full stop, apostrophe and question mark. Modifiers follow, and every other UK keyboard symbol is on Symbols.'
            : config.keyboardLayout === 'simpleQwerty'
              ? 'Three familiar letter rows: Q W E R T Y U I O P Backspace; A S D F G H J K L Enter; Shift Z X C V B N M full stop; then Ctrl, Alt, Space and Windows (Option and Command on a Mac). Every other UK keyboard symbol is on Symbols.'
              : 'Familiar QWERTY key rows: Q W E R T Y U I O P; A S D F G H J K L apostrophe; Z X C V B N M comma full stop question mark. Every other UK keyboard symbol is on Symbols.' }}
        />
        <p className="setting-note">Simple QWERTY keeps the letter order with fewer keys on its Letters page. Common letters first puts frequent English letters and Space early in row scanning. The keyboard footer cycles through all three layouts; the choice is saved.</p>
      </SettingGroup>
      <p className="setting-note">The Symbols page shows every symbol on the main keys of a UK keyboard as its own key, and each key types what it shows. The scanning keyboard spaces prose punctuation for you: after a full stop, comma, question mark or exclamation mark on Letters or Symbols it adds a space. Other symbols type exactly as shown, and the full stop on Numbers stays a decimal point. Period, question mark, and exclamation mark capitalize the next letter.</p>
    </>}
    {(area === 'keyboard' || area === 'mouse' || area === 'menu') && <SettingGroup title="Panel position" description="The keyboard, the mouse panel, Home and the action menus share one position. Choose it from Position on the keyboard or mouse panel.">
      <Toggle label="Move away from the pointer" checked={config.panelAvoidsPointer} disabled={disabled} onChange={value => update('panelAvoidsPointer', value)} />
      <p className="setting-note">While the pointer is over a panel or menu, it moves to the other side of the screen. It returns when the pointer moves away.</p>
      {(platform === 'windows' || platform === 'macos') && appSettings && onSettingsChange && <>
        <Toggle label="Show carer toolbar" checked={appSettings.carerToolbar} onChange={value => onSettingsChange({ ...appSettings, carerToolbar: value })} />
        <p className="setting-note">A small bar beside the panel with Settings, Pause and Close panel buttons, for someone helping with a mouse. Switches never scan it. This choice stays on this computer.</p>
      </>}
    </SettingGroup>}
    {area === 'keyboard' && <SettingGroup title="Suggestions" description="Show word suggestions while typing with the scanning keyboard.">
      <Toggle label="Word prediction" checked={config.wordPrediction} disabled={disabled} onChange={value => update('wordPrediction', value)} />
      <p className="setting-note">Uses an offline English word database and an on-device language model. Suggestions appear as you type and after a space. The keyboard shows when suggestions are loading, unavailable, or have no match. If prediction fails, choose Retry predictions on the keyboard; typing still works. Your typing never leaves this computer.</p>
      {appSettings && onSettingsChange && <LearnedWords settings={appSettings} onSettingsChange={onSettingsChange} disabled={!config.wordPrediction} />}
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
    {area !== 'point' && field('pattern', 'Item scanning', locked => <OptionGroup<ItemScanning> legend="Item scanning" columns="three" disabled={locked} value={itemScanning(effective)} options={(['grouped', 'rowGroups', 'linear'] as const).map(value => ({ value, label: itemScanningLabels[value] }))} onChange={chooseItemScanning}
      note={{ summary: 'Groups, then items chooses a row before an item. Rows, groups, then items also splits a row of seven or more items, such as a keyboard row, into two to four smaller groups. One item at a time visits each item directly. Point scanning uses its own line and grid modes.' }} />)}

    <SettingGroup title="Appearance" description="Choose a highlight that is easy to see.">
    {area === 'shared' && <OptionGroup<PanelSize>
      legend="Panel size" disabled={disabled} value={settings.panelSize ?? 'medium'}
      options={[{ value: 'small', label: 'Small' }, { value: 'medium', label: 'Medium' }, { value: 'large', label: 'Large' }]}
      onChange={panelSize => update('scanPreferences', { ...settings, panelSize })}
      note={{ summary: 'Sizes the keyboard, the mouse panel, Home and the action menus. Each panel fits its controls and shrinks to fit the screen.' }}
    />}
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
        // Row groups are part of the item scanning setting, not one of their own.
        const count = Object.entries(settings[key] ?? {}).filter(([name, value]) => value != null && name !== 'rowGroups').length;
        return <Button type="button" className="scanner-area-card" data-area={key} key={key} aria-label={`Customise ${names[key].toLowerCase()}`} onClick={() => openArea(key)}>
          <strong>{names[key]}<ChevronRight size={22} aria-hidden="true" /></strong>
          <span>{options.automatic ? `Automatic · ${options.intervalMs / 1000}s` : 'Manual'} · {options.direction === 'forward' ? 'Forward' : 'Reverse'}</span>
          <span>{itemScanningLabels[itemScanning(options)]}</span>
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

function LearnedWords({ settings, onSettingsChange, disabled }: { settings: AppSettings; onSettingsChange: (next: AppSettings) => void; disabled: boolean }) {
  const [confirming, setConfirming] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const forget = async () => {
    setConfirming(false);
    try {
      const done = await api.forgetLearnedWords();
      setStatus(done ? 'Learned words forgotten.' : 'Learned words will be forgotten in a moment, as soon as the keyboard stops using them.');
    } catch (error) {
      setStatus(String(error));
    }
  };
  return <>
    <Toggle label="Learn from my typing" checked={settings.learnWordPrediction} disabled={disabled} onChange={value => onSettingsChange({ ...settings, learnWordPrediction: value })} />
    <p className="setting-note">Words and phrases you type with the scanning keyboard are suggested sooner. What is learned stays on this computer and is not synced. It is word counts, not a copy of your text, but it can include anything you type, such as names or passwords.</p>
    {confirming
      ? <div role="group" aria-label="Confirm forgetting learned words">
        <p className="setting-note">Forget every word learned on this computer? Suggestions return to the built-in English words. This cannot be undone.</p>
        <div className="row-actions"><Button type="button" className="primary danger" onClick={() => void forget()}>Forget learned words</Button><Button type="button" className="secondary" onClick={() => setConfirming(false)}>Keep them</Button></div>
      </div>
      : <Button type="button" className="secondary danger" onClick={() => { setStatus(null); setConfirming(true); }}>Forget learned words…</Button>}
    {status && <p role="status" className="setting-note">{status}</p>}
  </>;
}
