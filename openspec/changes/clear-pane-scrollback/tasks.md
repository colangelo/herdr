## 1. Terminal-level clear

- [ ] 1.1 Add a scrollback-clear method on the pane runtime / Ghostty `Terminal` wrapper (`src/pane/terminal.rs` near `scroll_reset`, or `src/ghostty/mod.rs`) that injects `ESC[3J` through the existing write path
- [ ] 1.2 Ensure the injection is not subject to the droid `CSI 3J` strip (`src/pane/osc.rs`), so a herdr-originated clear reaches the emulator even when `droid` is the foreground job
- [ ] 1.3 Confirm the visible screen and process are untouched, and that the call is a safe no-op on empty scrollback / alt screen

## 2. Socket API

- [ ] 2.1 Add `Method::PaneClearScrollback` (`pane.clear`) variant in `src/api/schema.rs`
- [ ] 2.2 Add the handler in `src/app/api/panes.rs` that resolves the target pane id and triggers the terminal-level clear; return a not-found error for unknown pane ids
- [ ] 2.3 Compare `src/protocol/wire.rs::PROTOCOL_VERSION` against the latest released tag; bump only if not already ahead, and update hardcoded protocol expectations/fixtures if bumped

## 3. TUI keybinding

- [ ] 3.1 Add `NavigateAction::ClearScrollback` in `src/app/input/navigate.rs` and dispatch it to the `pane.clear` client path for the focused pane
- [ ] 3.2 Add a `clear_scrollback` field to `KeysConfig` (`src/config/model.rs`) next to `edit_scrollback`; resolve default-bound-vs-unbound per design Open Questions and wire it in the keymap table

## 4. CLI

- [ ] 4.1 Add a `clear` subcommand under `herdr pane` in `src/cli/pane.rs` / `src/cli/spec.rs` accepting an explicit pane id and `--current`
- [ ] 4.2 Send the `pane.clear` request over the session socket and report success/failure

## 5. Tests

- [ ] 5.1 Unit test the terminal-level clear: scrollback emptied, visible screen preserved, no-op on empty scrollback
- [ ] 5.2 Test that the herdr-originated clear succeeds under a simulated `droid` foreground state (not suppressed by the passthrough filter)
- [ ] 5.3 Test the `pane.clear` handler: success for a valid pane, not-found for an unknown pane id
- [ ] 5.4 Add a keymap test for the `clear_scrollback` config field mapping to `NavigateAction::ClearScrollback`; if the protocol was bumped, update protocol fixtures

## 6. Docs and validation

- [ ] 6.1 Document `pane.clear` in `docs/next/website/src/content/docs/socket-api.mdx`, `herdr pane clear` in `cli-reference.mdx`, and the keybinding (if bound) in `keyboard.mdx` / `configuration.mdx`
- [ ] 6.2 Run `just check` and fix any failures
- [ ] 6.3 Dogfood live: fill a pane's scrollback, run the clear via keybinding, `herdr pane clear`, and the socket method; confirm scrollback is gone while the visible screen and process remain
