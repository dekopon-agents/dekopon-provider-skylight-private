# Skylight private provider

> **Exploration only—opt-in, unofficial, private, unsupported, not production.** This provider is
> not affiliated with, endorsed by, or supported by Skylight. Skylight publishes no public API for
> these routes; they can change without notice, and using them may violate applicable terms or
> trigger account enforcement. Source-backed contracts and synthetic tests are not official support.
> **Successful native broker HTTP validation is pending.** No subscription tier is inferred from HTTP errors.

`skylight-private` is a broker-only Rust component implementing API
`dekopon.dev/provider/v1alpha1`. Its eight ordered capabilities are independently grantable,
Medium-risk and read-only. All requests use `https://app.ourskylight.com/api`:

| Capability | GET route | Small projection |
|---|---|---|
| `skylight.private.account.read` | `/user` | `account.id` only (not full Python `whoami`) |
| `skylight.private.frames.list` | `/frames` | At most 32 sorted frame IDs and optional names |
| `skylight.private.categories.list` | `/frames/{frameId}/categories` | `categories`: IDs, labels, profile flags, family-member linkage; no inferred identity |
| `skylight.private.calendar.events.list` | `/frames/{frameId}/calendar_events?date_min=…&date_max=…&timezone=…[&include=…]` | `events`: source times, summaries, recurrence, multiple category linkages |
| `skylight.private.lists.list` | `/frames/{frameId}/lists` | `lists`: labels, kind/color/flags, linked included items |
| `skylight.private.lists.read` | `/frames/{frameId}/lists/{listId}` | `list`: metadata and linked included items; section count only |
| `skylight.private.list.items.list` | `/frames/{frameId}/lists/{listId}/list_items` | `items`: labels, source status, section, position/draft, list linkage |
| `skylight.private.tasks.list` | `/frames/{frameId}/chores?after=…&before=…&include_late=…&include_up_for_grabs=…&filter=linked_to_profile` | `tasks`: source status/dates/recurrence, separate assignment and completion-category linkage |

The legacy manifest description remains `Unsupported private Skylight account and frame reads over
broker HTTP`. The command word is `skylight`. Original account/frames inputs and outputs are unchanged.

## Command word and strict inputs

| Argv after `skylight` | Required JSON input keys |
|---|---|
| `account` | exactly `{}` |
| `frames` | exactly `{}` |
| `categories --frame ID` | `frameId` |
| `events --frame ID --from DATE --to DATE --tz ZONE [--include categories,calendar_account,event_notification_setting]` | `frameId`, `dateMin`, `dateMax`, `timezone`; optional singleton string `include` |
| `tasks --frame ID --after DATE --before DATE [--include-late true\|false] [--include-up-for-grabs true\|false] [--filter linked_to_profile]` | `frameId`, `after`, `before`; optional booleans `includeLate`, `includeUpForGrabs`, singleton string `filter` |
| `lists --frame ID` | `frameId` |
| `list-show --frame ID --list ID` | `frameId`, `listId` |
| `list-items --frame ID --list ID` | `frameId`, `listId` |
| `--help`, `-h`, `help` | renders help, exit 0; no proposal or HTTP |

Selectors and date/zone values are required strings; optional inclusion flags are typed as above. Flags can be reordered, but not repeated; no positional IDs,
`--flag=value`, implicit frame, per-command help, stdin JSON, or transport escape hatch is accepted.
Unknown/missing/duplicate flags and invalid values return a fixed sanitized usage error, exit 2.
Piped input is ignored. Every input schema has `additionalProperties:false`; duplicate wire JSON
members fail closed. Account/frames still reject every selector and flag. The SDK `clap` feature
remains off. Proposals undergo the same constraint/Cedar/broker authorization as direct invocation;
parsing never grants authority or contacts HTTP.

Frame/list selectors must be 1–128 ASCII letters, digits, `_` or `-`. Slash, percent, dot segments,
query delimiters and control characters are rejected. Query values are percent-encoded (including
`/` in timezones and `+` in `Etc/GMT+5`). No URL, method, arbitrary include, page, cursor, header, body, token,
assignee or completion selector is accepted. Unknown capability takes precedence over invalid JSON.

### Calendar boundaries and interpretation

Dates are real Gregorian `YYYY-MM-DD` values (years 0001–9999). Require **1–31 days between** `--from`
and `--to`; equal dates, reversed ranges and longer intervals fail before HTTP. This is provider
policy, not a discovered upstream limit. For a daily query, explicitly pass that day's date and the
next date. The provider now sends **date-only** values with the explicit `--tz` value, matching
the verified browser client (previous midnight suffix construction was not live verified).
It does **not** claim verified upstream inclusive/exclusive boundaries, expand recurrence, clip
multiday events, convert offsets or compute “today.” DST changes do not trigger a guessed 24-hour
UTC conversion. Timezones are syntactically bounded to 1–128 ASCII letters/digits/`_`/`+`/`-`/`/`,
with no empty slash segments; there is no embedded IANA database to verify zone existence. Callers
must choose a valid upstream-supported zone; an absent frame timezone is not silently UTC.

Source `starts_at` and `ends_at` strings are preserved verbatim as `startsAt`/`endsAt` (at most
256 bytes each, otherwise invalid-response), not validated as timestamps or interpreted. This
preserves offsets, date-only all-day values and multiday crossings. Missing/null timing is unknown.
Missing/null `all_day` becomes `allDay:null`, distinct from `false` and `true`.

### Verified scoped parameters and Tasks interpretation

The calendar `include` input is optional for compatibility. If supplied, the **only accepted string**
is `categories,calendar_account,event_notification_setting`, exactly the successful browser request.
Individual tokens, reordered/subset CSV, singular Python mock `category`, arbitrary include paths,
and any other backend parameters are unsupported/unverified. Omission is not a verified server
default. Categories, lists, list detail and list-items expose **no query parameters** in the audited
client methods. No generic kwargs/JSON/query forwarding or pagination options are added. No frame
metadata endpoint is necessary: calendar still requires an explicit timezone; Tasks has no verified
timezone parameter. Account/frames semantics, including no include-deleted flag, remain unchanged.

`tasks` reads `/chores`, not lists or task-box. `after` and `before` are Gregorian date-only strings,
**0–31 days apart** (provider bound). Equal endpoints are verified for the UI-selected day. Use the
same selected date for both; do not substitute the calendar's next-day interval. Defaults for both
inclusion booleans are **false**, taken from current client source, not inferred server defaults.
The provider always sends both flags and `filter=linked_to_profile`; the optional filter input is
constrained to that singleton. All four boolean combinations were browser-observed working.
To match the observed Today inclusion settings, explicitly supply both flags as true:

```console
skylight tasks --frame FRAME_ID --after YYYY-MM-DD --before YYYY-MM-DD --include-late true --include-up-for-grabs true
```

The caller must supply the selected date: no clock, implicit timezone, or guessed “today.” Arbitrary
multi-day inclusivity, late-task lookback, set-theoretic inclusion guarantees, timezone/DST behavior,
recurrence expansion and occurrence-versus-series uniqueness remain unknown. No server completed,
skipped, profile/assignee or outstanding predicate is invented: the browser filters `complete`,
`skipped` and disabled profiles locally, with recurrence/date logic beyond a simple status test.

Task `status` preserves bounded source text: live `pending`/`complete`, source-only `skipped`, and
unknown future statuses. `group`, `series`, `start`, `completedAt` are bounded opaque source strings,
not occurrence IDs, parsed timestamps or completion guarantees. `completed_at` string/null was
observed, but its timestamp format is unknown. `completedOnState`, `recurringUntilState` and
`startTimeState` are `null-or-missing` or `unverified-non-null`; unverified non-null values are never
forwarded. Recurrence strings are retained without interpreting RRULEs. A completion-category
relationship is **not** the assignment category. List item `status` retains the legacy
`pending`/`completed`/null projection; new `sourceStatus` preserves unknown source text separately.
List-item status does not establish due date, assignee, or outstanding Tasks.

Task→`category`, event→`categories[]`, and category→nullable `family_member` linkage are retained.
Included records are hydrated into a bounded top-level `included` collection, keyed by **type and
ID**, not label. Each linkage has `included:true` only when that exact pair survives in the output;
`false` means unresolved (possibly locally omitted), **not unassigned**. Missing/null relationship
objects remain unknown; explicit `{data:null}` differs from an empty linkage array. Included
category stubs may lack family-member evidence. Never treat every category as a person, use a
completion category as assignment, or resolve duplicate labels to a partner. Family-member
attributes, notification/account attributes and unknown resource-specific fields are unverified;
`attributesCoverage` explicitly denotes a small known-field subset, not full hydration.

This can supply evidence for tentative household questions, **not a complete outstanding-today
answer, partner docket, trip itinerary, booking or agenda**. Empty/bounded responses, recurrence,
unknown statuses and unresolved linkage must remain visible limitations.

## Bounded contract and projection

Each valid invocation constructs exactly one HTTPS GET, with empty body and only these ordered
guest headers. It never retries or follows pagination; the native broker follows no redirects.

```text
accept: application/json
user-agent: dekopon-skylight-private-provider/0.1 (+https://github.com/dekopon-agents/dekopon)
```

The legacy user-agent is intentionally retained. The guest never sets `authorization`, `cookie`, or
`content-type`. Only the broker injects the destination-bound bearer. Only status 200 is decoded;
response headers/content type are ignored and response bodies are capped at 262,144 bytes.

Account retains only `data.id`: a non-empty string of at most 128 UTF-8 bytes. Frames validate all
records before retaining the 32 lexicographically smallest IDs. IDs must be unique/non-empty and
at most 128 bytes. Missing attributes means unnamed; present attributes must be an object. Present
frame name/label must be strings (including null being invalid); nonempty name wins over label.
Names over 256 bytes use a UTF-8-safe prefix plus `…` within 256 bytes; `nameTruncated` is always
present. Frame `truncated` retains its legacy record-omission semantics, **not** upstream completeness.

Household outputs retain at most 64 ID-sorted primary records; list detail returns one object
(or null with `truncated:true` if its whole projection cannot fit). Included resources independently
retain at most 64 deterministically sorted type/ID pairs. Linkage arrays independently retain at
most 16 sorted pairs; recurrence arrays retain at most 16 source-order strings. Every known
field and duplicate ID/member is validated even in a discarded tail. JSON objects cannot be
positional arrays. Missing attributes defaults to unknown fields, but present nonobject attributes
fails. Missing/null optional fields are unknown; malformed known types fail closed. Unknown
fields, unrecognized relationships and pagination links are not forwarded. A shared typed attribute
decoder validates recognized names/types even for discarded records. Useful included categories,
list items, avatars and typed resource stubs are retained, without URLs/emails or arbitrary raw JSON.
`meta.sections` is counted only: populated element schema is unknown; nonempty sections imply local
omission (`truncated:true`), empty arrays yield zero, missing/null yields null.
Source text (labels, summaries, descriptions, statuses, section) is capped to 256 UTF-8 bytes with `…`; `textTruncated` marks
local text loss per record. Record/top-level `truncated` also marks rule/linkage count loss;
`rruleTruncated`/`recurrenceSetTruncated` and linkage-array `truncated` make those omissions explicit.
Top-level `truncated` marks any local text/count/byte/section omission; whole
records are omitted to keep projected JSON at most 32,640 bytes and the SDK envelope below 32 KiB.
All new outputs include `projection:"typed-subset"`, `coverage:"bounded-response"`,
`upstreamCompleteness:"unknown"` and explicit linkage/included/section coverage states.
Combined output budgeting accounts for JSON escaping once per retained candidate; included records
are dropped first, then primary records, and surviving linkage flags are resolved afterward.
**`truncated:false` means no local loss, not an exhaustive upstream result.** No pagination is fetched.
Household text remains untrusted sensitive metadata, not instructions or declassified data.

Failures redact URI, status detail, headers, body, credentials and transport/parser text:

| Code | Message |
|---|---|
| `unknown-capability` | `unsupported Skylight private capability` |
| `invalid-input` (account/frames) | `input must be exactly an empty object` |
| `invalid-input` (new reads) | `input must match the bounded Skylight read schema` |
| `invalid-request` | `could not construct the fixed Skylight request` |
| `http-failed` | `broker HTTP request failed` |
| `invalid-response` | `the private API returned an invalid response` |
| `reauth-required` | `the broker credential must be replaced or re-enrolled` |
| `forbidden` | `Skylight refused this private API read` |
| `not-found` | `the private API resource was not found` |
| `rate-limited` | `the private API rate limit was reached` |
| `unexpected-status` | `the private API returned an unexpected status` |

## Required broker-only authority

A deployment must explicitly opt in to **each desired capability** with an owner-authored constraint
set. The examples below preserve the original grants; use exactly the same limits for each new
capability ID in the table above, granting only those needed. Neither manifest imports nor this
repository grant authority. No capability implicitly grants any other. The native host follows no redirects. Do not relax the exact
host, method, one-request maximum, HTTPS-only posture, ten-second deadline, or byte ceilings.

```yaml
constraintSets:
  skylight.private.account.read:
    provider: skylight-private
    effect: read-only
    risk: Medium
    credential: skylight-poc-bearer
    constraints:
      timeoutMs: 10000
      maxOutputBytes: 32768
      http:
        allowedHosts: [app.ourskylight.com]
        allowedMethods: [GET]
        maxRequests: 1
        maxRequestBytes: 4096
        maxResponseBytes: 262144
        allowPlaintextLoopback: false
  skylight.private.frames.list:
    provider: skylight-private
    effect: read-only
    risk: Medium
    credential: skylight-poc-bearer
    constraints:
      timeoutMs: 10000
      maxOutputBytes: 32768
      http:
        allowedHosts: [app.ourskylight.com]
        allowedMethods: [GET]
        maxRequests: 1
        maxRequestBytes: 4096
        maxResponseBytes: 262144
        allowPlaintextLoopback: false
```

The operator must obtain a disposable, short-lived access token out of band, only where authorized,
and install it solely in the owner-only broker credential store. Use a non-PII symbolic name and
bind it to exactly `app.ourskylight.com`. The broker validates the request before injecting
`Authorization: Bearer …` where the guest cannot observe it. Credential values must never enter
source, fixtures, inputs, outputs, errors, logs, traces, evidence, audit, or names.

The credential store is static and loaded at startup. Replacement can require a broker restart.
This component implements no OAuth, login, PKCE, callback, MFA/CAPTCHA handling, refresh, token
cache, rotation, revocation, expiry persistence, enrollment, or pre-expiry renewal. One bearer may
expose multiple accounts or frames; the upstream bearer remains the final resource boundary.

Never add this provider to default catalogs, images, policies, credentials, packages, or
deployments.

## Provenance

This standalone extraction is based on
[`dekopon-agents/dekopon@62d2185f9ec6fee61f2689197b274a9b4947659f`](https://github.com/dekopon-agents/dekopon/commit/62d2185f9ec6fee61f2689197b274a9b4947659f).
The implementation originated in Dekopon PR
[`#120`](https://github.com/dekopon-agents/dekopon/pull/120), squash commit `89dfac98`, with
original branch commits `a853fb26`, `9092095f`, and `e4d5da24`.

The checked monorepo baseline artifact was 246,823 bytes with SHA-256
`1cbb23fd13dc6296e38e360b81c2ce22b73d7605edd81295a05d99d1b8236f0a`. That value records
provenance only; it is not the hash of a standalone release.

Route and response-shape evidence is pinned to
[`joshuaswarren/pyskylight`](https://github.com/joshuaswarren/pyskylight) commit
`69e4576b9035d71aacda9ade7a4afea05a663e94`. The complete upstream MIT notice, and every other
dependency's license, is disclosed in the CycloneDX SBOM the shared release workflow attaches to
each release; there is no tracked `THIRD_PARTY_NOTICES.md` in this repository. This is a native
Rust reimplementation; Python is not embedded.

## Python support matrix (all 146 registered commands)

Audit pinned to `joshuaswarren/pyskylight@69e4576b9035d71aacda9ade7a4afea05a663e94`:
[CLI registrations](https://github.com/joshuaswarren/pyskylight/blob/69e4576b9035d71aacda9ade7a4afea05a663e94/src/pyskylight/cli.py),
[client routes](https://github.com/joshuaswarren/pyskylight/blob/69e4576b9035d71aacda9ade7a4afea05a663e94/src/pyskylight/client.py),
[models](https://github.com/joshuaswarren/pyskylight/blob/69e4576b9035d71aacda9ade7a4afea05a663e94/src/pyskylight/models.py),
[client tests](https://github.com/joshuaswarren/pyskylight/blob/69e4576b9035d71aacda9ade7a4afea05a663e94/tests/test_client.py),
[additional tests](https://github.com/joshuaswarren/pyskylight/blob/69e4576b9035d71aacda9ade7a4afea05a663e94/tests/test_full_coverage.py).
There are **54 ordinary reads, 88 actions, 2 auth commands and 2 sensitive GET helpers**. “Ordinary
read” is a source classification, not a safety grant. Actions include mutations, code/link generation
and uploads; even sensitive GET helpers are not ordinary household reads. No mutation/auth command
or sensitive helper is implemented. Tasks is now supported through the browser-verified chores
route, not an extra Python registration. All names below are Python registrations, not aliases.

**Partial** means source-compatible route with intentionally bounded arguments/output. Provider
`events` supports Python's named frame/from/to/tz/include parameters with a stricter evidenced
include singleton and date-only bounded range. Provider detail/items use `--list ID` rather than
Python's positional ID. There is no implicit configured frame. Chores parameters come from the
verified browser/current public client, not Python's opaque kwargs. Browser observations establish
working reads, not successful native broker HTTP. The separate list-items GET is **source-only**;
list index and populated detail included items were browser-observed. Every test fixture is synthetic.

| Python command | Source class | Provider support |
|---|---|---|
| `login` | Auth | Not implemented; out of scope |
| `logout` | Auth | Not implemented; out of scope |
| `whoami` | Read | Partial: `account`, ID only |
| `frames` | Read | Partial: bounded IDs/names; no include-deleted |
| `frame` | Read | Not implemented; out of scope |
| `events` | Read | Partial: bounded dates/zone, optional verified include, source times/recurrence/category linkage |
| `event-add` | Action | Not implemented; out of scope |
| `event-update` | Action | Not implemented; out of scope |
| `event-delete` | Action | Not implemented; out of scope |
| `categories` | Read | Partial: labels/flags/family-member linkage; no inferred profiles |
| `meal-categories` | Read | Not implemented; out of scope |
| `recipes` | Read | Not implemented; out of scope |
| `recipe` | Read | Not implemented; out of scope |
| `create-recipe` | Action | Not implemented; out of scope |
| `delete-recipe` | Action | Not implemented; out of scope |
| `update-recipe` | Action | Not implemented; out of scope |
| `grocery-add` | Action | Not implemented; out of scope |
| `plan` | Read | Not implemented; out of scope |
| `plan-add` | Action | Not implemented; out of scope |
| `plan-update` | Action | Not implemented; out of scope |
| `plan-remove` | Action | Not implemented; out of scope |
| `lists` | Read | Partial: metadata and linked included items |
| `list-add` | Action | Not implemented; out of scope |
| `chores` | Read | Partial: `tasks`, verified dates/flags/filter and typed source chores |
| `plan-show` | Read | Not implemented; out of scope |
| `list-show` | Read | Partial: metadata/included items, section count, `--list` selector |
| `list-items` | Read | Partial: typed labels/source status/section/linkage; route source-only |
| `category` | Read | Not implemented; out of scope |
| `update-meal-category` | Action | Not implemented; out of scope |
| `chore-add` | Action | Not implemented; out of scope |
| `chore-add-multiple` | Action | Not implemented; out of scope |
| `chore-update` | Action | Not implemented; out of scope |
| `chore-complete` | Action | Not implemented; out of scope |
| `chore-delete` | Action | Not implemented; out of scope |
| `list-create` | Action | Not implemented; out of scope |
| `list-update` | Action | Not implemented; out of scope |
| `list-delete` | Action | Not implemented; out of scope |
| `list-item-update` | Action | Not implemented; out of scope |
| `list-item-complete` | Action | Not implemented; out of scope |
| `list-item-delete` | Action | Not implemented; out of scope |
| `list-items-delete` | Action | Not implemented; out of scope |
| `list-item-move` | Action | Not implemented; out of scope |
| `list-items-section` | Action | Not implemented; out of scope |
| `category-add` | Action | Not implemented; out of scope |
| `category-find-or-create` | Action | Not implemented; out of scope |
| `category-update` | Action | Not implemented; out of scope |
| `category-delete` | Action | Not implemented; out of scope |
| `plan-instances` | Read | Not implemented; out of scope |
| `plan-instance-update` | Action | Not implemented; out of scope |
| `calendars` | Read | Not implemented; out of scope |
| `calendar-account` | Read | Not implemented; out of scope |
| `calendar-account-update` | Action | Not implemented; out of scope |
| `calendar-link` | Sensitive GET helper | Not implemented; out of scope |
| `webcals` | Read | Not implemented; out of scope |
| `webcal-add` | Action | Not implemented; out of scope |
| `source-calendars` | Read | Not implemented; out of scope |
| `source-calendar` | Read | Not implemented; out of scope |
| `source-calendar-update` | Action | Not implemented; out of scope |
| `source-calendar-delete` | Action | Not implemented; out of scope |
| `source-calendar-default` | Action | Not implemented; out of scope |
| `events-search` | Read | Not implemented; out of scope |
| `countdowns` | Read | Not implemented; out of scope |
| `event-invitees` | Read | Not implemented; out of scope |
| `event-notifications` | Read | Not implemented; out of scope |
| `event-notifications-update` | Action | Not implemented; out of scope |
| `reminder-notification` | Read | Not implemented; out of scope |
| `reminder-notification-update` | Action | Not implemented; out of scope |
| `source-calendar-categorize` | Action | Not implemented; out of scope |
| `category-categorize` | Action | Not implemented; out of scope |
| `task-box` | Read | Deferred: not proven Tasks |
| `task-box-add` | Action | Not implemented; out of scope |
| `task-box-update` | Action | Not implemented; out of scope |
| `task-box-delete` | Action | Not implemented; out of scope |
| `routines` | Read | Deferred: routines contract out of subset |
| `routine-add` | Action | Not implemented; out of scope |
| `routine-update` | Action | Not implemented; out of scope |
| `routine-delete` | Action | Not implemented; out of scope |
| `routines-reorder` | Action | Not implemented; out of scope |
| `rewards` | Read | Not implemented; out of scope |
| `reward` | Read | Not implemented; out of scope |
| `reward-add` | Action | Not implemented; out of scope |
| `reward-update` | Action | Not implemented; out of scope |
| `reward-delete` | Action | Not implemented; out of scope |
| `reward-redeem` | Action | Not implemented; out of scope |
| `reward-unredeem` | Action | Not implemented; out of scope |
| `reward-points` | Read | Not implemented; out of scope |
| `reward-points-set` | Action | Not implemented; out of scope |
| `messages` | Read | Not implemented; out of scope |
| `message` | Read | Not implemented; out of scope |
| `message-delete` | Action | Not implemented; out of scope |
| `messages-delete` | Action | Not implemented; out of scope |
| `messages-copy` | Action | Not implemented; out of scope |
| `message-caption` | Action | Not implemented; out of scope |
| `message-likes` | Read | Not implemented; out of scope |
| `message-like` | Action | Not implemented; out of scope |
| `message-unlike` | Action | Not implemented; out of scope |
| `message-comments` | Read | Not implemented; out of scope |
| `message-comment-add` | Action | Not implemented; out of scope |
| `message-comment-delete` | Action | Not implemented; out of scope |
| `photo-upload` | Action | Not implemented; out of scope |
| `upload-credentials` | Sensitive GET helper | Not implemented; out of scope |
| `albums` | Read | Not implemented; out of scope |
| `album-add` | Action | Not implemented; out of scope |
| `album-update` | Action | Not implemented; out of scope |
| `album-delete` | Action | Not implemented; out of scope |
| `album-messages` | Read | Not implemented; out of scope |
| `album-message-ids` | Read | Not implemented; out of scope |
| `album-add-photos` | Action | Not implemented; out of scope |
| `album-remove-photos` | Action | Not implemented; out of scope |
| `month-in-review` | Read | Not implemented; out of scope |
| `month-in-reviews` | Read | Not implemented; out of scope |
| `avatars` | Read | Not implemented; out of scope |
| `colors` | Read | Not implemented; out of scope |
| `activities` | Read | Not implemented; out of scope |
| `devices` | Read | Not implemented; out of scope |
| `device` | Read | Not implemented; out of scope |
| `device-update` | Action | Not implemented; out of scope |
| `device-delete` | Action | Not implemented; out of scope |
| `device-activation-code` | Action | Not implemented; out of scope |
| `device-reset` | Action | Not implemented; out of scope |
| `alarms` | Read | Not implemented; out of scope |
| `alarm-add` | Action | Not implemented; out of scope |
| `alarm-update` | Action | Not implemented; out of scope |
| `alarm-delete` | Action | Not implemented; out of scope |
| `members` | Read | Not implemented; out of scope |
| `member-invite` | Action | Not implemented; out of scope |
| `member-approve` | Action | Not implemented; out of scope |
| `member-remove` | Action | Not implemented; out of scope |
| `member-update` | Action | Not implemented; out of scope |
| `household-config` | Read | Not implemented; out of scope |
| `household-config-update` | Action | Not implemented; out of scope |
| `weather` | Read | Not implemented; out of scope |
| `geolocation` | Read | Not implemented; out of scope |
| `share-link` | Action | Not implemented; out of scope |
| `plus-status` | Read | Not implemented; out of scope |
| `frame-rename` | Action | Not implemented; out of scope |
| `frame-settings` | Action | Not implemented; out of scope |
| `frame-hide` | Action | Not implemented; out of scope |
| `frame-activation-code` | Action | Not implemented; out of scope |
| `ai-intents` | Read | Not implemented; out of scope |
| `ai-intent` | Read | Not implemented; out of scope |
| `ai-intent-create` | Action | Not implemented; out of scope |
| `ai-intent-approve` | Action | Not implemented; out of scope |
| `ai-intent-retry` | Action | Not implemented; out of scope |
| `ai-intent-undo` | Action | Not implemented; out of scope |
| `ai-intent-items` | Read | Not implemented; out of scope |

## Build and verification

The only compiler provenance is exact Rust 1.98.1. Component composition uses exact `wasm-tools`
1.259.0. All Dekopon dependencies are exact crates.io 0.18.0 pins; there are no Git, path,
symlink, submodule, or adjacent-checkout dependencies. The repository owns only its composed WIT
world. Its three dependency WIT mirrors (provider, HTTP, and asset) are checked byte-for-byte
against the resolved crates.io 0.18.0 package contents, not trusted by a local hash.

```console
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo deny --all-features check bans licenses sources advisories
../provider-workflows/build.sh
DEKOPON_PROVIDER_COMPONENT=$PWD/skylight-private-provider.wasm \
  cargo test --locked --workspace
```

This mirrors the gates `ci / validate` in
[`dekopon-agents/provider-workflows`](https://github.com/dekopon-agents/provider-workflows) runs:
formatting, dependency policy, clippy for the host and for `wasm32-unknown-unknown`, the WIT
mirror check, the component build, a raw wasmtime smoke test, the SBOM, and a byte-for-byte
rebuild from a clean checkout. There is no local `scripts/` directory or `build.sh` in this
repository anymore; `../provider-workflows/build.sh` is a sibling checkout of the shared
workflows repository (see its own README for the exact clone step CI uses).

`build.sh` writes only ignored files under `target/` and the repository root: an intermediate
core module, the component (`skylight-private-provider.wasm`), and its checksum. The CycloneDX
SBOM is a release asset the shared workflow generates in CI, not a local or tracked file. Each
immutable `v<version>` source tag identifies the exact gated component in a GitHub prerelease
carrying the component and its checksum, attested with `actions/attest-build-provenance`, and the
same bytes are stored as the sole `application/wasm` layer at
`ghcr.io/dekopon-agents/provider-skylight-private:<version>` — the OCI tag drops the leading `v`
— under artifact type `application/vnd.dekopon.provider.v1+wasm`. Verify a downloaded component
with `gh attestation verify skylight-private-provider.wasm --owner dekopon-agents`. The first tag,
`v0.1.0`, never left draft; `v0.2.0` is the first published prerelease. A `-` suffix on the tag
(for example `v0.4.0-pre.1`) marks a prerelease. Neither artifact is a supported production
distribution, and neither adds the provider to a default catalog, image, policy, credential set,
package, or deployment.

All behavior tests use synthetic in-memory responses. The component-host test implements the sole
buffered HTTP WIT import in memory and opens no socket; unexpected streaming or asset calls fail
the test. The real broker host is used only for pre-network authority, method, request-budget and credential-destination refusal; successful native broker HTTP cannot be safely mocked without
changing the fixed production URI. No test contacts Skylight, a public host, DNS, or loopback, and
no captured response fixture is permitted.

The finished component must export only `describe`, `invoke`, and `run-command`, import exactly
`dekopon:http/client@1.1.0`, and import no WASI, filesystem, environment, clock, random, socket,
JavaScript, or other ambient interface. The immediate host refuses it because that host provides no
HTTP import. HTTP 1.1.0's asset types require the asset WIT mirror at build time; unused stream
and asset imports are eliminated from this buffered-only component. There is no tracked `security/` directory in this repository; the committed fuel,
memory, and timeout ceilings and measured headroom now live only as the `MAX_*`/`TIMEOUT*`
constants and the `committed_component_limits_are_exact` / `committed_broker_limits_are_exact`
tests in `tests/component_host.rs` and `tests/broker_host.rs`.

### Read-contract evidence and resource-budget increment

The scoped Python audit is pinned above. Two authorized browser studies observed successful GETs
on the designated calendar (19 prior schema observations, then 14 parameter/hydration observations).
The latter verified Tasks flags/filter, same-day selected dates, calendar date-only/include requests,
and populated list detail. Current public page source symbols `getChores`, `getCalendarEvents`,
`getCategories`, `getLists`, `getList`, `getListItems` corroborated the parameter inventory; the exact
bundle asset path was not retained. That inventory is **not exhaustive backend documentation**.
Individual calendar include subsets, server omission/default rules, pagination, completion transitions,
recurrence-instance semantics, section schemas and complete household coverage remain unverified.
Successful browser reads use the browser's separate authentication context; native broker HTTP
success still requires separately authorized validation. No credential/session data is in fixtures.

The typed Tasks/relationships/included decoder requires a parent-approved component artifact ceiling
of **589,824 bytes (576 KiB)**, replacing 393,216 bytes. This changes only the artifact-size gate:
**128,000,000 fuel, 32 MiB guest memory, ten seconds, one GET, 4 KiB input/request, 256 KiB response,
and <32 KiB component output are unchanged**. No dependencies, compiler profiles, shared CI or
ambient imports are added. Minimal records retain lazy boxed attributes/relationships and only the
final selected resources are projected, preserving descending-input resource bounds.
