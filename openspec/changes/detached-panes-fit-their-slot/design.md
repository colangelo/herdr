## Decisions

**No-client size = last client's size.** `handoff_client_size` becomes
`last_client_size`: set from the effective size on every sync with a foreground
client, kept when the client goes, and seeded from a handoff manifest.
`detached_size()` is `last_client_size`, or `headless_size` when absent. This
supersedes #94's "the first attach ends the carried size": the attaching client's
size replaces it, and outlives its detach.

**Resize in the no-client render.** `render_and_stream` without render targets
passes `resize_panes = true`. The first-frame-only guard existed so that a
detached server would not shrink panes to 120×40. Because the no-client size is
now the size panes already have, that guard is no longer needed. A fresh server
and a handoff without a carried size still start at the headless default, as
before.

## Risks

- A small client (a phone, a narrow remote window) that detaches leaves its size
  as the no-client size. That is the size its panes already have today.
