# Skylight private provider

> **Exploration only—opt-in, unofficial, private, unsupported, not production.** This provider is
> not affiliated with, endorsed by, or supported by Skylight. Skylight publishes no public API for
> these routes; they can change without notice, and using them may violate applicable terms or
> trigger account enforcement. Source-backed contracts and synthetic tests are not official support.
> **Successful native broker HTTP validation is pending.** No subscription tier is inferred from HTTP errors.

`skylight-private` is a broker-only Rust component implementing API
`dekopon.dev/provider/v1alpha1`. Its thirty-five ordered capabilities are independently grantable.
The seventeen reads are Medium-risk and read-only (`ReadOnly`). The eighteen writes are
`ExternalWrite`: three calendar event writes, four list item writes, three list writes, four
category writes and four task (chore) writes. Creates, updates and completes are High risk; deletes are Critical. Because they are not
`ReadOnly`, no write ever joins the broker's `skylight-private:read-only` group.
**No write is granted or bound in any deployment, and whether Skylight accepts a write made with
the broker's token is unverified: no live write has been made.** Requests default to
`https://app.ourskylight.com/api`:

| Capability | Method and route | Small projection |
|---|---|---|
| `skylight-private.account.read` | `/user` | `account.id` only (not full Python `whoami`) |
| `skylight-private.frames.list` | `/frames` | At most 32 sorted frame IDs and optional names |
| `skylight-private.categories.list` | `/frames/{frameId}/categories` | `categories`: IDs, labels, profile flags, family-member linkage; no inferred identity |
| `skylight-private.calendar.events.list` | `/frames/{frameId}/calendar_events?date_min=…&date_max=…&timezone=…[&include=…]` | `events`: source times, summaries, recurrence, multiple category linkages |
| `skylight-private.lists.list` | `/frames/{frameId}/lists` | `lists`: labels, kind/color/flags, linked included items |
| `skylight-private.lists.read` | `/frames/{frameId}/lists/{listId}` | `list`: metadata and linked included items; section count only |
| `skylight-private.list.items.list` | `/frames/{frameId}/lists/{listId}/list_items` | `items`: labels, source status, section, position/draft, list linkage |
| `skylight-private.tasks.list` | `/frames/{frameId}/chores?after=…&before=…&include_late=…&include_up_for_grabs=…&filter=linked_to_profile` | `tasks`: source status/dates/recurrence, separate assignment and completion-category linkage |
| `skylight-private.calendar.events.create` | POST `/frames/{frameId}/calendar_events` | `event`: the returned record, as the events projection; `{"ok":true}` on 204 |
| `skylight-private.calendar.events.update` | PUT `/frames/{frameId}/calendar_events/{eventId}` | `event`: the returned record; `{"ok":true}` on 204 |
| `skylight-private.calendar.events.delete` | DELETE `/frames/{frameId}/calendar_events/{eventId}` | `event` when a record is returned; `{"ok":true}` on 204 |
| `skylight-private.list.items.create` | POST `/frames/{frameId}/lists/{listId}/list_items` | `item`: the returned record, as the items projection; `{"ok":true}` on 204 |
| `skylight-private.list.items.update` | PUT `/frames/{frameId}/lists/{listId}/list_items/{itemId}` | `item`: the returned record; `{"ok":true}` on 204 |
| `skylight-private.list.items.complete` | PUT `/frames/{frameId}/lists/{listId}/list_items/{itemId}` (status only) | `item`: the returned record; `{"ok":true}` on 204 |
| `skylight-private.list.items.delete` | DELETE `/frames/{frameId}/lists/{listId}/list_items/{itemId}` | `item` when a record is returned; `{"ok":true}` on 204 |
| `skylight-private.lists.create` | POST `/frames/{frameId}/lists` | `list`: the returned record, as the list projection; `{"ok":true}` on 204 |
| `skylight-private.lists.update` | PUT `/frames/{frameId}/lists/{listId}` | `list`: the returned record; `{"ok":true}` on 204 |
| `skylight-private.lists.delete` | DELETE `/frames/{frameId}/lists/{listId}` (the list and its items) | `list` when a record is returned; `{"ok":true}` on 204 |
| `skylight-private.categories.create` | POST `/frames/{frameId}/categories` | `category`: the returned record, as the categories projection; `{"ok":true}` on 204 |
| `skylight-private.categories.find-or-create` | POST `/frames/{frameId}/categories/find_or_create` | `category`: the found or created record; `{"ok":true}` on 204 |
| `skylight-private.categories.update` | PATCH `/frames/{frameId}/categories/{categoryId}` | `category`: the returned record; `{"ok":true}` on 204 |
| `skylight-private.categories.delete` | DELETE `/frames/{frameId}/categories/{categoryId}[?reassign_to_id={categoryId}]` | `category` when a record is returned; `{"ok":true}` on 204 |
| `skylight-private.tasks.create` | POST `/frames/{frameId}/chores` | `task`: the returned record, as the tasks projection; `{"ok":true}` on 204 |
| `skylight-private.tasks.update` | PUT `/frames/{frameId}/chores/{taskId}` | `task`: the returned record; `{"ok":true}` on 204 |
| `skylight-private.tasks.complete` | PUT `/frames/{frameId}/chores/{taskId}/completions` | `task`: the returned record; `{"ok":true}` on 204 |
| `skylight-private.tasks.delete` | DELETE `/frames/{frameId}/chores/{taskId}` | `task` when a record is returned; `{"ok":true}` on 204 |

| `skylight-private.messages.list` | `/frames/{frameId}/messages` (first page only) | `messages`: IDs, captions, created times, linkage; never media URLs |
| `skylight-private.messages.read` | `/frames/{frameId}/messages/{messageId}` | `message`: the same fields for one message |
| `skylight-private.messages.likes.list` | `/frames/{frameId}/messages/{messageId}/all_likes` | `likes`: IDs, created times, linkage |
| `skylight-private.messages.comments.list` | `/frames/{frameId}/messages/{messageId}/comments` | `comments`: IDs, bodies, created times, linkage |
| `skylight-private.albums.list` | `/frames/{frameId}/albums` | `albums`: IDs, titles, created times |
| `skylight-private.albums.messages.list` | `/frames/{frameId}/albums/{albumId}/messages` | `messages`: as `messages.list`, for one album |
| `skylight-private.albums.message-ids.list` | `/frames/{frameId}/albums/{albumId}/messages/all_ids` | `messageIds`: message IDs |
| `skylight-private.month-in-review.read` | `/frames/{frameId}/month_in_review` | `monthInReview`: ID, created time, linkage |
| `skylight-private.month-in-review.list` | `/frames/{frameId}/month_in_reviews` | `monthInReviews`: IDs, created times, linkage |

The reads' routes are GETs.

The manifest description is `Unsupported private Skylight household reads and calendar event, list,
list item, category and chore writes over broker HTTP` (originally `… account and frame reads over broker HTTP`). The command word is
`skylight`. Original account/frames inputs and outputs are unchanged.

## Owner base URL

The optional owner setting `providerSettings.skylight-private.baseUrl` defaults to
`https://app.ourskylight.com`:

```yaml
providerSettings:
  skylight-private:
    baseUrl: https://fixture.example.test/skylight
```

Every route appends `/api/...` to this base, preserving a configured path prefix and encoded
query values. SDK 0.75.0 validates the base as HTTP or HTTPS with a host and no userinfo, query,
fragment or whitespace. Malformed settings and unknown keys fail before HTTP. Omitted settings
or an empty settings object keep the default. Model inputs and CLI flags cannot set an origin.

The broker remains the destination-policy boundary: configure `allowedHosts` and credential
`destinations` for the chosen authority. A plaintext loopback recorder also requires an explicit
`allowPlaintextLoopback` grant. Changing the setting grants no network or credential authority.

`tests/cassettes/skylight-private/0001-GET-calendar-events.json` is an **authored synthetic
cassette v1 fixture**, derived from `src/household_tests.rs` calendar projection and encoded-query
cases. `0002-POST-calendar-events.json` through `0004-DELETE-calendar-event.json` (event writes)
`0005-POST-list-items.json` through `0008-DELETE-list-item.json` (list item writes),
`0009-POST-lists.json` through `0011-DELETE-list.json` (list writes),
`0012-POST-chores.json` through `0015-DELETE-chore.json` (task writes) and
`0016-POST-categories.json` through `0019-DELETE-category.json` (category writes) are authored
synthetic write exchanges.
All of them are labelled `provenance: synthetic`, and none was recorded against Skylight. It is not a vendor recording and contains no credentials, cookies, account identifiers,
or real household data. `tests/cassette.rs` verifies exact URI, method and headers through the
native request seam, and replays the same response through the real component. The component
harness's host match alone is not used as proof of path/query correctness. Both require no vendor
access; the component replay requires `DEKOPON_PROVIDER_COMPONENT` and never silently skips.

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
| `messages --frame ID`, `albums --frame ID`, `month-in-review --frame ID`, `month-in-reviews --frame ID` | `frameId` |
| `message`, `message-likes`, `message-comments` `--frame ID --message ID` | `frameId`, `messageId` |
| `album-messages`, `album-message-ids` `--frame ID --album ID` | `frameId`, `albumId` |
| `event-add --frame ID --summary TEXT [--starts-at TIME] [--ends-at TIME] [--all-day true\|false] [--tz ZONE] [--category ID]... [--rrule RULE]...` | `frameId`, `summary`; optional `startsAt`, `endsAt`, `allDay`, `timezone`, arrays `categoryIds`, `rrule` |
| `event-update --frame ID --event ID [--summary TEXT] [--starts-at TIME] [--ends-at TIME] [--all-day true\|false] [--tz ZONE]` | `frameId`, `eventId` and at least one optional field |
| `event-delete --frame ID --event ID` | `frameId`, `eventId` |
| `list-add --frame ID --list ID --label TEXT [--section TEXT]` | `frameId`, `listId`, `label`; optional `section` |
| `list-item-update --frame ID --list ID --item ID [--label TEXT] [--status completed\|pending] [--position N]` | `frameId`, `listId`, `itemId` and at least one optional field |
| `list-item-complete --frame ID --list ID --item ID [--status completed\|pending]` | `frameId`, `listId`, `itemId`; optional `status`, default `completed` |
| `list-item-delete --frame ID --list ID --item ID` | `frameId`, `listId`, `itemId` |
| `list-create --frame ID --label TEXT [--color COLOR] [--kind shopping\|to_do]` | `frameId`, `label`; optional `color`, `kind` |
| `list-update --frame ID --list ID [--label TEXT] [--color COLOR] [--kind shopping\|to_do]` | `frameId`, `listId` and at least one optional field |
| `list-delete --frame ID --list ID` | `frameId`, `listId` |
| `category-add --frame ID --label TEXT --color COLOR` | `frameId`, `label`, `color` |
| `category-find-or-create --frame ID --label TEXT --color COLOR` | `frameId`, `label`, `color` |
| `category-update --frame ID --category ID [--label TEXT] [--color COLOR]` | `frameId`, `categoryId` and at least one optional field |
| `category-delete --frame ID --category ID [--reassign-to ID]` | `frameId`, `categoryId`; optional `reassignToId` |
| `chore-add --frame ID --summary TEXT [--start DATE] [--reward-points N] [--category ID] [--recurring true\|false] [--rrule RULE]... [--up-for-grabs true\|false] [--emoji ICON] [--status pending\|complete]` | `frameId`, `summary`; optional `start`, `rewardPoints`, `categoryId`, `recurring`, `recurrenceSet`, `upForGrabs`, `emojiIcon`, `status` |
| `chore-update --frame ID --chore ID [--summary TEXT] [--start DATE] [--reward-points N] [--status pending\|complete] [--category ID] [--emoji ICON]` | `frameId`, `taskId` and at least one optional field |
| `chore-complete --frame ID --chore ID [--status complete\|pending] [--instance-date DATE] [--category ID]` | `frameId`, `taskId`; optional `status` (default `complete`), `instanceDate`, `categoryId` |
| `chore-delete --frame ID --chore ID` | `frameId`, `taskId` |
| `--help`, `-h`, `help` | renders help, exit 0; no proposal or HTTP |

Selectors and date/zone values are required strings; optional inclusion flags are typed as above. Flags can be reordered. Only `event-add`'s `--category` and `--rrule`, and `chore-add`'s `--rrule`, may repeat: each occurrence adds one array element, at most 16. Every other flag may appear once; no positional IDs,
`--flag=value`, implicit frame, per-command help, stdin JSON, or transport escape hatch is accepted.
Unknown/missing/duplicate flags and invalid values return a fixed sanitized usage error, exit 2.
Piped input is ignored. Every input schema has `additionalProperties:false`; duplicate wire JSON
members fail closed. Account/frames still reject every selector and flag. The SDK `clap` feature
remains off. Proposals undergo the same constraint/Cedar/broker authorization as direct invocation;
parsing never grants authority or contacts HTTP.

Frame/list/event/item/category/task selectors (including `reassignToId` and `categoryId`) must be 1–128 ASCII letters, digits, `_` or `-`. Slash, percent, dot segments,
query delimiters and control characters are rejected. Query values are percent-encoded (including
`/` in timezones and `+` in `Etc/GMT+5`). No URL, method, arbitrary include, page, cursor, header, body, token,
assignee or completion selector is accepted. Unknown capability takes precedence over invalid JSON.

### Calendar event writes

The writes take only typed, bounded fields, and every write input has `additionalProperties:false`:

- `summary` is 1–256 UTF-8 bytes, not blank, with no control characters.
- `startsAt` and `endsAt` are a Gregorian `YYYY-MM-DD` or that date with `THH:MM[:SS[.fraction]]`
  and an optional `Z` or `±HH:MM` (at most 64 bytes). They are sent as given, not converted.
- `allDay` is a boolean. `timezone` follows the calendar read's rule.
- `categoryIds` holds 1–16 unique selectors. `rrule` holds 1–16 printable-ASCII strings of 1–256
  bytes; the provider sends recurrence rules as given and does not interpret them.

Update takes neither categories nor recurrence, and must name at least one field. Delete takes
only its two selectors. The body is flat snake_case JSON (`summary`, `starts_at`, `ends_at`,
`all_day`, `timezone`, `category_ids`, `rrule`, in that order), and absent fields are omitted,
never sent as null. This follows `joshuaswarren/pyskylight`'s client and tests. No URL, method,
header, query, `extra` or `--json` passthrough exists.

### List item writes

The list item writes take the same bounded text rules as the event writes:

- `label` and `section` are 1–256 UTF-8 bytes, not blank, with no control characters.
- `status` is exactly `completed` or `pending`, the two values the items projection knows. Other
  source statuses are never written.
- `position` is an integer from 0 to 1,000,000.

`list-add` sends `label` and an optional `section`. `list-item-update` sends any of `label`,
`status` and `position`, and must name at least one. `list-item-complete` sends only `status`:
`completed` by default, or `pending` to reopen an item. `list-item-delete` sends no body. The
bodies are flat JSON in that field order, with absent fields omitted, following
`joshuaswarren/pyskylight`'s `add_list_item`, `update_list_item` and `complete_list_item`.
pyskylight's `extra` passthrough on update is not offered.

### Photo and message reads

The photo, message, album and month-in-review reads are unverified against the live API. Their
routes come from `joshuaswarren/pyskylight`'s client source, and no response shape for them is
documented, so each is projected through the same JSON:API envelope as the other reads:

- Each record keeps `id`, `type`, `createdAt`, known linkage, and `caption`, `title` or `body` when
  the source gives a string. Those three names are the ones pyskylight's message, album and comment
  write bodies use.
- Every other attribute is never read. That covers asset, thumbnail and share URLs, file names,
  dimensions, links and pagination tokens, so no media URL can reach the output.
- Included resources keep only their type and ID.
- `messages` sends no `page` or `page_token`, so only the first page is read, and pagination is
  never followed.
- A response that is not that envelope is `invalid-response`.

### List writes

`list-create` sends `label` and an optional `color` and `kind`. `list-update` sends any of these,
and must name at least one. `list-delete` sends no body and removes the list with its items.
Inputs are bounded as follows:

- `label` follows the item label rule.
- `color` is 1–32 ASCII letters, digits, `#`, `_` or `-`: a hex code or a palette name. Skylight's
  accepted colors are unverified.
- `kind` is exactly `shopping` or `to_do`, pyskylight's documented values.

pyskylight's `hide_from_frame` is not offered, because the read side reports a different flag
(`hide_on_device`). Neither is its `extra` passthrough.

### Task (chore) writes

The task writes go to the same `/chores` route the `tasks` read uses. Every request, writes
included, sends `skylight-api-version`.

- `chore-add` requires a `summary`. Its other fields are optional:
  - `start`: a Gregorian `YYYY-MM-DD` date.
  - `rewardPoints`: 0–1,000.
  - `categoryId`: an assignment category, as a selector.
  - `recurring`: a boolean.
  - `recurrenceSet`: 1–16 printable-ASCII rules of 1–256 bytes, not interpreted.
  - `upForGrabs`: a boolean.
  - `emojiIcon`: 1–32 bytes with no whitespace or control characters.
  - `status`: `pending` or `complete`, the live-observed values.
- `chore-update` changes `summary`, `start`, `rewardPoints`, `status`, `categoryId` or `emojiIcon`,
  and must name at least one.
- `chore-complete` PUTs to `/completions` with `status`: `complete` by default, or `pending` to
  reopen. It also takes an optional `instanceDate`, for one occurrence of a series, and an optional
  completion `categoryId`.
- `chore-delete` sends no body. Which occurrences of a recurring series it removes is unverified.

`start_time`, `recurring_until` and the completion's `instance_time` are not offered: the `tasks`
read discards them and their formats are unverified. Delete's `apply_to` is not offered either,
because its values are unknown. Neither are `extra` or pyskylight's `create_multiple`.

### Category writes

Categories carry family-member linkage on the read side, so these writes change what other reads
show.

- `category-add` and `category-find-or-create` both require `label` and `color`, with the list
  rules. Find-or-create finds or creates one category by label and color. Which fields Skylight matches on
  is unverified, so a new color with an existing label may create a second category.
- `category-update` sends a PATCH with either field, and must name at least one.
- `category-delete` sends no body. Its optional `reassignToId` is a bounded selector and becomes
  `?reassign_to_id=…`. That is the only query string any write sends: a bounded selector, never free text. Reassigning a
  category to itself is refused before HTTP. A category can be a family member's profile
  (`linkedToProfile`). Without `--reassign-to`, what Skylight does with the category's events and
  chores is unverified.
- pyskylight's `extra` passthrough is not offered.

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

Each valid invocation constructs exactly one request. A read is an HTTPS GET with an empty body
and only these ordered guest headers. It never retries or follows pagination; the native broker
follows no redirects.

```text
accept: application/json
user-agent: dekopon-skylight-private-provider/0.1 (+https://github.com/dekopon-agents/dekopon)
skylight-api-version: 2026-05-01
```

A write sends its capability's fixed method (POST, PUT, PATCH or DELETE) with the same three headers,
then `content-type: application/json`. DELETE sends that header too, with an empty body. Whether
Skylight's write endpoints require the API-version header is unverified. The legacy user-agent is
intentionally retained. Skylight serves `include_up_for_grabs=true` chores only to requests that
name this API version.

The guest never sets `authorization` or `cookie`, and never sets `content-type` on a read. Only
the broker injects the destination-bound bearer. A read decodes only status 200. Response headers
and content type are ignored, and response bodies are capped at 262,144 bytes.

A write decodes a 200 or 201 as the returned record, and a 204 yields exactly
`{"ok":true}`. A 2xx means the write applied, so a write never fails after one. A 200 or 201 whose
body is missing, malformed or over the cap yields `{"ok":true,"recordProjected":false}`, not
`invalid-response`, so that a caller does not retry and repeat the write.

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
records are omitted to keep projected JSON at most 32,640 bytes and the stdout line below 32 KiB.
All new outputs include `projection:"typed-subset"`, `coverage:"bounded-response"`,
`upstreamCompleteness:"unknown"` and explicit linkage/included/section coverage states.
Combined output budgeting accounts for JSON escaping once per retained candidate; included records
are dropped first, then primary records, and surviving linkage flags are resolved afterward.
**`truncated:false` means no local loss, not an exhaustive upstream result.** No pagination is fetched.
Household text remains untrusted sensitive metadata, not instructions or declassified data.

Failures redact URI, status detail, headers, body, credentials and transport/parser text:

| Code | Message |
|---|---|
| `unknown-capability` | `the provider has no such capability` (or a broker-level unknown-ID refusal) |
| `invalid-input` | `the input does not match the capability's input schema` |
| `invalid-request` | `could not construct the fixed Skylight request` |
| `http-failed` | `broker HTTP request failed` |
| `invalid-response` | `the private API returned an invalid response` |
| `reauth-required` | `the broker credential must be replaced or re-enrolled` |
| `forbidden` | `Skylight refused this private API call` |
| `not-found` | `the private API resource was not found` |
| `rate-limited` | `the private API rate limit was reached` |
| `unexpected-status` | `the private API returned an unexpected status` |

## Required broker-only authority

A deployment must explicitly opt in to **each desired capability** with an owner-authored constraint
set. List only the capability IDs you need under `capabilities`; an unlisted capability does not
run, and `{}` inherits the provider-level limits. Neither manifest imports nor this repository grant
authority. No capability implicitly grants any other. The native host follows no redirects. Do not
relax the exact host, method, one-request maximum, HTTPS-only posture, ten-second deadline, or byte
ceilings.

```yaml
capabilities:
  skylight-private:
    credential: skylight-private-bearer
    constraints:
      timeoutMs: 10000
      http:
        allowedHosts: [app.ourskylight.com]
        allowedMethods: [GET]
        maxRequests: 1
        maxRequestBytes: 4096
        maxResponseBytes: 262144
    capabilities:
      skylight-private.account.read: {}
      skylight-private.frames.list: {}
      skylight-private.categories.list: {}
      skylight-private.calendar.events.list: {}
      skylight-private.lists.list: {}
      skylight-private.lists.read: {}
      skylight-private.list.items.list: {}
      skylight-private.tasks.list: {}
```

This example lists the eight original reads under a GET-only constraint set, which is today's deployment. The nine photo, message and album reads are `ReadOnly`, so they join `skylight-private:read-only` and its permits. Like any capability, though, each runs only once it has its own `capabilities:` entry and a GET binding, and this example lists none of them.
Under it every write is refused before HTTP. To run a write, an owner must add three things:

- the write's own `capabilities:` entry, whose `http.allowedMethods` override names exactly its
  method (`POST`, `PUT` or `DELETE`), as the deployment's `gh.issue.comment` entry does for POST;
- a secret-map binding for that method and path;
- a Cedar permit naming the capability.

This repository grants none of them.

The provider-level `credential:` above is the legacy static path: the operator installs a short-lived
access token out of band in the owner-only broker credential store, bound to exactly
`app.ourskylight.com`, and replaces it by hand when it expires.

The broker can instead keep the token alive itself. With core `dekopon-brokerd` v0.48.0 and chart
0.35.0, an owner-authored `oauth2Refresh` secret-map DRN with `refresh: scheduled` and a
broker-private `seedPath`, plus `defaultSecretUse` on each constraint set in place of `credential:`,
gives the broker the refresh, idle rotation, seed adoption and sealing.
[`docs/dekopon-skylight.md`](docs/dekopon-skylight.md) has that configuration and the owner's
reconnect procedure. The same provider artifact serves both paths; neither needs a provider setting
or release.

Either way the broker validates the request before injecting `Authorization: Bearer …` where the
guest cannot observe it. This component itself still implements no OAuth, login, PKCE, callback,
MFA/CAPTCHA handling, refresh, token cache, rotation, revocation, expiry persistence, enrollment, or
pre-expiry renewal; it maps an upstream 401 to `reauth-required` and nothing more. Credential values
must never enter source, fixtures, inputs, outputs, errors, logs, traces, evidence, audit, or names.
One bearer may expose multiple accounts or frames; the upstream bearer remains the final resource
boundary.

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
and uploads; even sensitive GET helpers are not ordinary household reads. The only mutations
implemented are the three calendar event writes (`event-add`, `event-update`, `event-delete`) and
the four list item writes (`list-add`, `list-item-update`, `list-item-complete`,
`list-item-delete`), the three list writes (`list-create`, `list-update`, `list-delete`) and
the four category writes (`category-add`, `category-find-or-create`, `category-update`,
`category-delete`), and the four task writes (`chore-add`, `chore-update`, `chore-complete`,
`chore-delete`). They are inert: none is granted or bound in any deployment, and Skylight's acceptance of them is
unverified. No other mutation, no auth command and no sensitive helper is implemented. Tasks is now supported through the browser-verified chores
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
| `event-add` | Action | Partial: `event-add` → `calendar.events.create`; typed fields only, no `--json`/extra; ungranted, live acceptance unverified |
| `event-update` | Action | Partial: `event-update` → `calendar.events.update`; summary/times/all-day/zone only; ungranted, live acceptance unverified |
| `event-delete` | Action | Partial: `event-delete` → `calendar.events.delete`; ungranted, live acceptance unverified |
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
| `list-add` | Action | Partial: `list-add` → `list.items.create`; label and optional section; ungranted, live acceptance unverified |
| `chores` | Read | Partial: `tasks`, verified dates/flags/filter and typed source chores |
| `plan-show` | Read | Not implemented; out of scope |
| `list-show` | Read | Partial: metadata/included items, section count, `--list` selector |
| `list-items` | Read | Partial: typed labels/source status/section/linkage; route source-only |
| `category` | Read | Not implemented; out of scope |
| `update-meal-category` | Action | Not implemented; out of scope |
| `chore-add` | Action | Partial: `chore-add` → `tasks.create`; typed fields; no `start_time`, `recurring_until` or extra; ungranted, live acceptance unverified |
| `chore-add-multiple` | Action | Not implemented; out of scope |
| `chore-update` | Action | Partial: `chore-update` → `tasks.update`; summary, start, points, status, category, icon; ungranted, live acceptance unverified |
| `chore-complete` | Action | Partial: `chore-complete` → `tasks.complete`; status, instance date, category; no `instance_time`; ungranted, live acceptance unverified |
| `chore-delete` | Action | Partial: `chore-delete` → `tasks.delete`; no `apply_to`; ungranted, live acceptance unverified |
| `list-create` | Action | Partial: `list-create` → `lists.create`; label, color, `shopping`/`to_do` kind; no hide flag or extra; ungranted, live acceptance unverified |
| `list-update` | Action | Partial: `list-update` → `lists.update`; label, color, kind; ungranted, live acceptance unverified |
| `list-delete` | Action | Partial: `list-delete` → `lists.delete`; ungranted, live acceptance unverified |
| `list-item-update` | Action | Partial: `list-item-update` → `list.items.update`; label, `completed`/`pending` status, position; no `extra`; ungranted, live acceptance unverified |
| `list-item-complete` | Action | Partial: `list-item-complete` → `list.items.complete`; `completed` or `pending`; ungranted, live acceptance unverified |
| `list-item-delete` | Action | Partial: `list-item-delete` → `list.items.delete`; ungranted, live acceptance unverified |
| `list-items-delete` | Action | Not implemented; out of scope |
| `list-item-move` | Action | Not implemented; out of scope |
| `list-items-section` | Action | Not implemented; out of scope |
| `category-add` | Action | Partial: `category-add` → `categories.create`; label and color; no extra; ungranted, live acceptance unverified |
| `category-find-or-create` | Action | Partial: `category-find-or-create` → `categories.find-or-create`; ungranted, live acceptance unverified |
| `category-update` | Action | Partial: `category-update` → `categories.update` (PATCH); label, color; ungranted, live acceptance unverified |
| `category-delete` | Action | Partial: `category-delete` → `categories.delete`; optional bounded `--reassign-to`; ungranted, live acceptance unverified |
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
| `messages` | Read | Partial: `messages` → `messages.list`; typed IDs/text/linkage, never media URLs; first page only; response shape unverified |
| `message` | Read | Partial: `message` → `messages.read`; typed IDs/text/linkage, never media URLs; first page only; response shape unverified |
| `message-delete` | Action | Not implemented; out of scope |
| `messages-delete` | Action | Not implemented; out of scope |
| `messages-copy` | Action | Not implemented; out of scope |
| `message-caption` | Action | Not implemented; out of scope |
| `message-likes` | Read | Partial: `message-likes` → `messages.likes.list`; typed IDs/text/linkage, never media URLs; first page only; response shape unverified |
| `message-like` | Action | Not implemented; out of scope |
| `message-unlike` | Action | Not implemented; out of scope |
| `message-comments` | Read | Partial: `message-comments` → `messages.comments.list`; typed IDs/text/linkage, never media URLs; first page only; response shape unverified |
| `message-comment-add` | Action | Not implemented; out of scope |
| `message-comment-delete` | Action | Not implemented; out of scope |
| `photo-upload` | Action | Not implemented; out of scope |
| `upload-credentials` | Sensitive GET helper | Not implemented; out of scope |
| `albums` | Read | Partial: `albums` → `albums.list`; typed IDs/text/linkage, never media URLs; first page only; response shape unverified |
| `album-add` | Action | Not implemented; out of scope |
| `album-update` | Action | Not implemented; out of scope |
| `album-delete` | Action | Not implemented; out of scope |
| `album-messages` | Read | Partial: `album-messages` → `albums.messages.list`; typed IDs/text/linkage, never media URLs; first page only; response shape unverified |
| `album-message-ids` | Read | Partial: `album-message-ids` → `albums.message-ids.list`; typed IDs/text/linkage, never media URLs; first page only; response shape unverified |
| `album-add-photos` | Action | Not implemented; out of scope |
| `album-remove-photos` | Action | Not implemented; out of scope |
| `month-in-review` | Read | Partial: `month-in-review` → `month-in-review.read`; typed IDs/text/linkage, never media URLs; first page only; response shape unverified |
| `month-in-reviews` | Read | Partial: `month-in-reviews` → `month-in-review.list`; typed IDs/text/linkage, never media URLs; first page only; response shape unverified |
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
1.259.0. The SDK, testkit, and broker crates are pinned to published core 0.75.0.
The SDK owns the provider world and stdio bindings; this repository keeps no WIT mirror.
The buffered HTTP import is `dekopon:http/client@1.1.0`; no HTTP 1.2 import is used.

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
formatting, dependency policy, clippy for the host and for `wasm32-unknown-unknown`,
the component build, component validation and the SBOM. Its byte-for-byte rebuild from a
clean checkout is paused in provider-workflows; `build.sh` itself is unchanged and reproducible. There is no local `scripts/` directory or `build.sh` in this
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

Tests use synthetic response bodies. Native tests run without a socket; real-component tests use
the testkit's pinned loopback HTTPS origin and broker authorization, never contact Skylight or a
public host, and assert private response fields never appear on stdout. The write smoke and
method tests in `tests/broker_host.rs` drive the built component through the pinned broker host to
a plaintext loopback consumer under `allowPlaintextLoopback`. The consumer captures the complete
request (request line, headers and body), and a GET-only grant refuses every write before HTTP.
Model inputs cannot override the origin. Owner settings route positive real-component reads to the testkit's ephemeral HTTPS
origin; native assertions verify exact joined paths and queries. Real-component conformance, denied
broker calls and credential-destination refusals remain covered. Live vendor HTTP validation still
requires separately authorized follow-up. The SDK's typed export uses
`dekopon:provider@0.4.0` and `dekopon:stdio/streams@0.1.0`, with broker-owned HTTP; no WASI or
ambient filesystem, environment, clock, random, or socket imports are permitted.

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

The component artifact ceiling is **1,048,576 bytes (1 MiB)**. To stay under it,
`Cargo.toml`'s `[profile.release]` sets `opt-level = "z"` and `strip = true`. The embedded
license sections are kept; only the symbol name section is dropped, so wasm backtraces carry no
function names. The former 589,824-byte
(576 KiB) bound predated the SDK-owned stdio bindings and typed export, which add code without
adding ambient authority. This changes only the artifact-size gate: **128,000,000 fuel,
32 MiB guest memory, ten seconds, one request (a GET for reads, the fixed method for writes),
4 KiB input/request, 256 KiB response, and
<32 KiB component output are unchanged**. Native 20,000-record synthetic responses exercise
bounded projection and malformed tails; real-component broker refusals exercise the configured
host limits. Positive real-component HTTP uses synthetic loopback fixtures through owner settings. Minimal records retain lazy boxed attributes/relationships and only the final
selected resources are projected, preserving descending-input resource bounds.
