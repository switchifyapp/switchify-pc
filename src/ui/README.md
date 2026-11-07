# Shared UI patterns

Use these components when adding or changing a screen. Keep feature state and backend calls in the feature.

- `controls.tsx` provides `Button`, `IconButton`, `Input`, `Select`, `Textarea`, and `MoreOptions`. Every icon-only button needs a `label`; its artwork is decorative.
- `patterns.tsx` provides `PageHeader`, `SectionHeader`, `ActionRow`, `FormField`, and `FeedbackMessage`. Use `ActionRow` when the whole row performs one action. Keep rows with several separate actions as ordinary articles.
- `FormField` passes an ID, description IDs, and validation state to its control. Spread those props onto the input, select, or textarea so the label, help, and error stay connected.
- Use inline feedback for a field error, a notice for an operation error, and a banner for an app-wide error. Errors announce as alerts; ordinary status uses `tone="status"`.
- `Dialog` owns its portal, background blocking, focus containment, Tab cycling, and focus restoration. Supply an accessible name and an `onEscape` action when the dialog can be dismissed. Pairing needs an explicit Accept or Reject choice, so it has no Escape dismissal.
- Use `ConfirmDialog` for destructive actions and unsaved changes. It focuses the keep/cancel action first. A busy confirmation cannot be dismissed or submitted again.
- Setup uses `suspended` while pairing takes over, which also suspends nested dialogs. It uses `active={false}` while native switch learning owns the modal. `captureKeyboard` is reserved for native learning: it swallows keyboard edges, including Tab and Escape, while the native capture service handles cancellation.

Shared keyboard behavior is tested in `Dialog.test.tsx`. Keep feature tests for navigation, native learning, setup, pairing, and practice when moving a screen onto these components.
