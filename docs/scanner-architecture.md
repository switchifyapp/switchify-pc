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
The `rows` convenience constructor is for fixed row layouts: its positional row
IDs are not suitable when groups can reorder. Supply explicit groups in that case.
Empty branches are removed and singleton branches collapse as before.

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

## Following stages

Issue #801 adds validated shared settings and per-area overrides to this core.
Issue #802 adds a main-window React content adapter and foreground/modal ownership.
Those stages do not require individual screens to implement traversal or timers.
They must preserve the current input generation checks, cleanup, native focus,
point countdown cancellation and Remote behaviour.
