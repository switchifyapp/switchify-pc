# Scanner architecture

The scanner has four boundaries: content, traversal, presentation and execution.
All desktop scanning shares the Rust `Session<Technique>` lifecycle and
`scanning_runtime::Adapter` platform boundary. Local and Remote switches feed
that same session. No item scanner captures keys, runs native input, persists
settings or owns a timer thread.

## Adding an item scanner

Use `scan_items::ItemScanner<Action>` with a small typed action enum. Supply
`scan_tree::Node::Group` nodes with stable group IDs and `Leaf` action identities.
Group IDs must be unique among siblings; leaf identities must be unique within
their group. They must identify the operation, not its current label or bounds.
The `configured_rows` convenience constructor is for fixed row layouts: its positional row
IDs are not suitable when groups can reorder. Supply explicit groups in that case.
Empty groups are removed. Explicitly identified groups retain their identity even
with one child. Anonymous branches and fixed single-item rows collapse as before.

Call `handle` with normalized switch actions and `advance` with elapsed time only
when the containing session permits movement. The core owns the interval,
direction, completed passes, suspension and nested Back navigation. Read its
navigator to build a presentation. Do not add a second interval in the provider.
`Policy` preserves the existing menu/keyboard differences during migration:
menus resume their selection; keyboards resume at the root. Keyboard manual
steps reset completed passes; existing menu manual steps do not.

Use `replace` when action availability or layout structure changes. Identified
groups and leaves retain their selection through reordering; a removed target
returns to the nearest surviving ancestor's first item with a full interval.
Any changed content invalidates pending activation. Identical content preserves
both timing and pending activation. Update labels and bounds separately when the
available actions have not changed. Do not put captured text into action IDs.

Before asynchronous execution, call `begin_activation` and retain the returned
revision. While pending, the core does not advance or select. Complete with that
same revision; stale or repeated completions return false. Restart and content
replacement cancel pending activation. The runtime's existing input generation
checks remain authoritative across whole-session replacement, disconnect and
capture cancellation; an item revision is local to one scanner instance.

The tests in `scan_items` provide a platform-free example using editing actions,
including reordered content, cancellation, exactly-once completion and timing.

## Existing providers

- `scan_menu` supplies typed menu actions and tile geometry. It delegates traversal
  to the item scanner and retains its existing three-column layout and parent stack.
- `scan_keyboard` supplies keys, modifiers, pages, prediction availability and
  geometry. It delegates traversal and pending activation to the item scanner.
  Prediction tokens still guard insertion, and queued prediction replacement
  remains a keyboard policy. Unchanged suggestions never restart the interval.
- `point_scan` retains its continuous movement strategy and uses the shared
  navigator for grid scanning. `point_workflow` composes point, menus, countdown,
  keyboard and execution-error stages within one session.
- Native hosts render `Frame` without activating Switchify. Domain actions are
  executed through the existing typed workflow requests and platform adapters.

## Settings

`scan_preferences::Preferences` resolves shared defaults and optional per-area
values for point scanning, menus and the keyboard into `Resolved` options. The existing top-level automatic, interval and
colour fields remain the shared defaults for compatibility. Missing overrides
inherit them; removing an override restores inheritance. Direction, pass limit,
item pattern and highlight thickness have backward-compatible defaults.

Use `Config::resolved(Area)` for effective settings. The workflow supplies the
active area's automatic mode to the session, so one manual area does not stop
another area's automatic movement. Switch eligibility requires navigation keys
if any area is manual. Pass limit zero means unlimited; grouped and linear item
scanning share the same navigator and execution revision. Keyboard skipping of
unavailable predictions counts automatic wraps but never manual passes.

### After a selection

Two preferences decide how scanning continues after a selection that does something, such as typing a key, clicking or scrolling. Each has a shared value and an optional value for each area.

| Preference | Values | Meaning |
|---|---|---|
| `nextScan` | `standard`, `automatic`, `wait` | Whether scanning moves on by itself or waits for Select |
| `startFrom` | `standard`, `beginning`, `selection` | Whether scanning starts from the beginning or from the item selected |

`standard` is what each scanner did before the preferences existed, and is the default:

| Scanner | `nextScan` | `startFrom` |
|---|---|---|
| Point scanning | Waits after a click, a closing command or a drag | Always the beginning; the preference is ignored |
| Menus | Moves on after a scroll or media item | The item selected |
| Keyboard | Moves on after a key or suggestion | The beginning |
| Mouse panel | Moves on after an action | The beginning |

The beginning is the first row or item, or the last when the initial direction is reverse.

Rules that hold whatever is chosen:

- Selections that only navigate start from the beginning of what they open. These are opening a menu, page or panel, Back, Close, the position items and the keyboard's modifier and Caps keys.
- The menu's mode and display items start a new point scan. The menu's speed items stay where they are and keep scanning.
- The mouse panel's Speed, Monitor and Drag keys are actions and follow both preferences.
- A failed action and a pause at the pass limit resume from where they always did.
- A chosen suggestion returns to the beginning, because its row is replaced.
- `wait` has no effect in an area that is not automatic, except that point scanning always waits unless `automatic` is chosen.
- A point scan that starts by itself still stops at the pass limit.

A point scan that started by itself takes up whichever window is in front until a switch is used in it, because the click before it may have brought another window forward. From then on, a change of window ends the scan as usual. While such a scan has no window in front, it holds still for up to a second, then ends as usual.

`keyboardWaitAfterTyping`, saved by earlier versions, still makes the keyboard wait. It applies only while the keyboard has no `nextScan` of its own and the shared value is `standard`. Settings no longer offers or changes it; the saved value is kept.

A value that is not recognised is dropped on its own when the file is read, and the other scan settings are kept.

`ItemScanner::continue_after_selection` applies both preferences for item scanners. `Workflow::used` decides what follows a used point.

The existing configure API cancels a running scan and waits for Select. Native
frames carry effective colour and thickness; individual renderers do not resolve
settings. Point line/grid geometry and movement speed remain separate controls.
