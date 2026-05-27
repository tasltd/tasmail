# indymilter — Postfix DLP Milter Research (TMAIL-108)

## Goal
Build an out-of-process Rust milter that Postfix calls before queueing
outbound mail. The milter inspects headers / body / attachments and
returns `Status::Reject`, `Status::Tempfail`, or a header-tag action when
content matches a DLP rule in PostgreSQL.

Only loaded on self-hosted operator deployments (TASMail BYOK does NOT
ship Postfix at `mail.techatscale.io` — see `docs/SELF-HOST-MAIL-SERVERS.md`).

## Crate
- **Name:** `indymilter` 0.3.0 (Mar 4, 2024)
- **License:** GPL-3.0-or-later (compatible — TASMail backend is MIT but
  the milter binary can be a separate GPL artefact since it's not
  statically linked into anything we ship as a library)
- **Pure async** — no libmilter C dependency
- **Runtime:** tokio 1.32+, async-trait 0.1, tracing 0.1

## API shape

```rust
use indymilter::{Callbacks, Context, SocketInfo, Status};
use tokio::{net::TcpListener, signal};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:8895").await.unwrap();
    let callbacks: Callbacks<MyState> = Callbacks::new()
        .on_connect(|ctx, _hostname, sock| Box::pin(handle_connect(ctx, sock)))
        .on_mail(|ctx, args| Box::pin(handle_mail(ctx, args)))
        .on_rcpt(|ctx, args| Box::pin(handle_rcpt(ctx, args)))
        .on_header(|ctx, name, value| Box::pin(handle_header(ctx, name, value)))
        .on_body(|ctx, chunk| Box::pin(handle_body(ctx, chunk)))
        .on_eom(|ctx| Box::pin(handle_eom(ctx)));
    indymilter::run(listener, callbacks, Default::default(), signal::ctrl_c())
        .await
        .unwrap();
}
```

- `Status::Continue` — keep processing
- `Status::Reject` — Postfix replies 5xx; mail is dropped
- `Status::Tempfail` — Postfix replies 4xx; sender retries
- `Status::Accept` — short-circuit accept (skip remaining checks)
- `Status::Discard` — silently drop

State is per-connection generic `T: Default + Send + Sync` on `Context<T>`.
Body chunks arrive as `&[u8]` and may be partial — accumulate in connection
state, then run the DLP scan on `on_eom`.

## Postfix wiring

`smtpd_milters` is a comma-separated list. Existing config has
`smtpd_milters = inet:127.0.0.1:11332` (rspamd). Append the DLP milter:

```
smtpd_milters = inet:127.0.0.1:11332, inet:127.0.0.1:8895
non_smtpd_milters = $smtpd_milters
milter_protocol = 6
milter_default_action = accept   # fail-open: don't bounce mail if milter is down
```

`milter_default_action = accept` deliberately fails-open so a milter
crash doesn't take down outbound mail. Operators who want strict DLP
should set it to `tempfail` so retries happen.

## Sources

- [indymilter on docs.rs](https://docs.rs/indymilter/latest/indymilter/) — API reference: Callbacks, Context, Status, run()
- [indymilter on lib.rs](https://lib.rs/crates/indymilter) — version 0.3.0, dependencies, minimal example
- [Postfix MILTER_README](https://www.postfix.org/MILTER_README.html) — smtpd_milters, milter_default_action, milter_protocol semantics
- [Sendmail milter API](https://www.milter.org/developers/api/) — original protocol indymilter implements

## Why a separate binary, not a backend feature

- BYOK send goes via the user's own SMTP server (no Postfix in the loop). DLP for BYOK is enforced inside the backend `smtp_service` before relaying.
- The milter is **only** for the self-host path where Postfix accepts mail on port 25/587 and relays/delivers locally. Adding `indymilter` to the main backend would pull GPL deps into the MIT crate.
- Running it as its own systemd unit lets operators disable DLP without touching the API server.
