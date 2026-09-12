# Switch assignments

In Settings → Switches, add a switch, enter its name, learn its physical key by pressing and releasing it, then save the new switch. Assign Select, Next, Previous, Reverse direction, Stop scanning or Pause / resume. Each key belongs to one switch; several switches can run the same action. Existing assignments save automatically. Disable scanning before editing. Learning ends when Settings loses focus, the panel closes, or Escape is pressed.

The normal action runs on release. Hold actions are offered in their configured order. The interval defaults to 1 second and accepts 250–5000 milliseconds. Each interval offers the next action in a nonactivating desktop prompt; the last remains selected. Release executes only the offered action. Movement freezes while any switch is held. The first pressed switch owns the gesture; overlapping presses do not activate additional actions.

Stop resets the scan but leaves switches enabled. Select starts again. Pause preserves position. Reverse changes direction without stepping. Automatic scanning requires Select; manual scanning also requires Next and Previous. Hold assignments count toward those requirements.

Escape disables capture immediately. The emergency hold timeout is at least 4 seconds and extends to (longest hold list + 2) × interval when hold actions exist. Settings shows the resulting duration. Heartbeat loss, capture failure, overflow, Android connection and shutdown cancel gestures without executing release actions.

## Storage and integration

The separate switch-settings.json uses schema version 1. Existing point-scan keys migrate once into four named assignments with empty hold lists. Fresh installs persist an empty list. Malformed or unsupported settings are reported without overwriting them. Saves use atomic replacement. Scanning stays off at startup.

Existing point-scan commands and event fields remain compatible for geometry and automatic mode. Legacy key fields remain readable; changing them is rejected with a direction to Settings → Switches. New settings and learning commands are restricted to the main window capability and check its label.

USAHP owns native Windows and macOS capture, suppression, learning, physical key state, release draining and emergency cancellation. Switchify embeds its typed API and drains generation-tagged physical edges. It opens no USAHP socket. Output still uses Switchify's existing input adapters. The pinned upstream extension is [usahp-core PR17](https://github.com/usahp/usahp-core/pull/17).

The reusable gesture engine handles normal and ordered hold actions. The shared scan controller dispatches them to Session<T>. Native prompts are click-through and nonactivating.

Automated tests use fake events and never send desktop input. Manual checks should cover physical suppression and learning, focus retention, hold timing, emergency exits, capture loss, permission changes, display changes and shutdown. Use npm run macos:run for macOS permission testing with the established app identity. This version supports keyboard switches and scanning actions.
