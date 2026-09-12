# Switch assignments

In Settings → Switches, each switch is a row showing its name, key and actions; Edit expands it. Add switch starts learning immediately: press and release the physical key, then enter a name and choose actions, then save. Learning can also be restarted from Change key. Assign Select, Next, Previous, Reverse direction, Stop scanning or Pause / resume. Each key belongs to one switch; several switches can run the same action. Existing assignments save automatically. Disable scanning before editing. Learning ends when Settings loses focus, the panel closes, or Escape is pressed.

The normal action runs on release. Hold actions are offered in their configured order. The interval defaults to 1 second and accepts 250–5000 milliseconds; five presets are shown and the rest sit behind an exact-interval select. The editor previews the time at which each hold action is offered. Each interval offers the next action in a nonactivating desktop prompt; the last remains selected. Release executes only the offered action. Movement freezes while any switch is held. The first pressed switch owns the gesture; overlapping presses do not activate additional actions.

Stop resets the scan but leaves switches enabled. Select starts again. Pause preserves position. Reverse changes direction without stepping. Automatic scanning requires Select; manual scanning also requires Next and Previous. Hold assignments count toward those requirements.

Escape disables capture immediately. The emergency hold timeout is at least 4 seconds and extends to (longest hold list + 2) × interval when hold actions exist. Settings shows the resulting duration. Heartbeat loss, capture failure, overflow, Android connection and shutdown cancel gestures without executing release actions.

## Storage and integration

The separate switch-settings.json uses schema version 1. Existing point-scan keys migrate once into four named assignments with empty hold lists. Fresh installs persist an empty list. Malformed or unsupported settings are reported without overwriting them. Saves use atomic replacement. Scanning stays off at startup.

Existing point-scan commands and event fields remain compatible for geometry and automatic mode. Legacy key fields remain readable; changing them is rejected with a direction to Settings → Switches. New settings and learning commands are restricted to the main window capability and check its label.

Switchify owns local input through `switch_input`, a platform adapter that supplies generation-tagged press/release events. There is no USAHP runtime or dependency. Output still uses Switchify's existing input adapters. The capture state and macOS event tap were adapted from MIT-licensed code; attribution is retained beside the source.

Windows reserves unmodified assigned keys and Escape with `RegisterHotKey`, uses `MOD_NOREPEAT` for press deduplication, and receives releases through Raw Input on a dedicated message thread. Startup fails and rolls back all registrations if an assigned key cannot be reserved. F12 is unavailable because Windows reserves it for the debugger. During learning, only keys successfully reserved for that capture can be learned; a key reserved by another app will not be learned. Disable, emergency cancellation and heartbeat loss release unused reservations on the input thread. Keys already consumed stay reserved until release, so holding a cancelled switch cannot leak autorepeat. App exit removes all reservations.

Windows does not provide complete suppression through this adapter: key releases can reach other applications, and combinations with Shift, Ctrl, Alt or Windows are not reserved. Use plain switch keys. Raw Input alone does not start scan actions. macOS retains its event-tap capture and suppression behavior. Physical Windows activation, hold behavior, background operation and cleanup need manual verification on each supported setup.

The reusable gesture engine handles normal and ordered hold actions. The shared scan controller dispatches them to Session<T>. Native prompts are click-through and nonactivating.

Automated tests use fake events and never send desktop input. Manual checks should cover physical suppression and learning, focus retention, hold timing, emergency exits, capture loss, permission changes, display changes and shutdown. Use npm run macos:run for macOS permission testing with the established app identity. This version supports keyboard switches and scanning actions.
