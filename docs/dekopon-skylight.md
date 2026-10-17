# Skylight credentials through a broker-refreshed DRN

This provider performs no OAuth. It never sees an access token, a refresh token, a seed, or a
record. The Dekopon broker owns all of it: it keeps the OAuth record, refreshes it, rotates the
refresh token while the provider is idle, adopts a new seed, seals a dead token family, and injects
`Authorization: Bearer …` into the one GET this provider builds.

The existing provider artifact works unchanged. There is no provider setting, rebuild or release for
this path. The broker side needs core `dekopon-brokerd` v0.48.0 and Helm chart 0.35.0:
`defaultSecretUse`, `seedPath` and sealing arrived in 0.47.0, scheduled refresh in 0.48.0, and
`broker.privateFiles` and `broker.oauth2Records` in chart 0.35.0.

The broker contract is in core and is not repeated here:

- [`oauth2Refresh` source](https://github.com/dekopon-agents/dekopon/blob/v0.48.0/docs/secrets.md#oauth2refresh):
  record format, refresh, `lazy`/`scheduled`, seed adoption, sealing, failure categories, record reset.
- [Two independent policies](https://github.com/dekopon-agents/dekopon/blob/v0.48.0/docs/secrets.md#two-independent-policies)
  and the [private map](https://github.com/dekopon-agents/dekopon/blob/v0.48.0/docs/secrets.md#private-map).
- [Current privileged broker foundation](https://github.com/dekopon-agents/dekopon/blob/v0.48.0/docs/security-model.md#current-privileged-broker-foundation):
  where injected material enters a request and why the guest cannot read it.
- [Broker failure events](https://github.com/dekopon-agents/dekopon/blob/v0.48.0/docs/observability.md#broker-failure-events):
  `broker.credential.sealed`, `broker_credential_record_invalid`, `broker_credential_scheduled_failed`.

## Who does what

| | Provider (guest) | Broker |
|---|---|---|
| Request | Builds one fixed GET to `app.ourskylight.com`; never sets `authorization` (the host rejects a guest-supplied one) | Checks method, host, path and query against the binding before injecting |
| Which secret | Proposes none: no `secretUse`, no DRN, no `--secret` operand | Attaches the capability's owner-authored `defaultSecretUse` when the proposal names no secret |
| Authority | None | Capability decision, a separate Cedar `secret.use` decision on the exact DRN, and the private binding must all allow |
| OAuth | None: no login, refresh, token cache, rotation, expiry, seed or record | `oauth2Refresh`: record lock, bare refresh-token grant, atomic rotation, scheduled idle refresh, seed adoption, sealing |
| Upstream 401 | Maps it to `reauth-required` | Does not seal on an API 401; only token-endpoint reauthorization codes and bootstrap reflection seal the record |
| Credential bytes | Never present in input, output, errors or memory | Enter the request only inside the native HTTP engine; absent from audit, spans and evidence |

If the broker cannot resolve the DRN (sealed, missing, malformed, or a refresh that failed), the
invocation fails with a fixed broker failure before the provider runs. No Skylight API request is
sent.

## Operator configuration

Four pieces, all owner-authored. The DRN, Secret name and paths below are one operator's choices;
pick your own. Placeholders in angle brackets are never real values.

### 1. Chart values

`broker.privateFiles` copies a Secret key into a broker-only `0600` file under a broker-owned `0700`
directory at pod start. The copy only changes on a pod rollout, so list the seed Secret in the
Reloader annotation. `broker.oauth2Records` keeps its default: a broker-owned subdirectory of the
state claim mounted at `/var/lib/dekopon/oauth2-records`, so the record survives restarts.

```yaml
broker:
  privateFiles:
    - file: skylight-refresh-seed
      existingSecret: dekopon-skylight-seed
      existingSecretKey: refresh-seed
deploymentAnnotations:
  secret.reloader.stakater.com/reload: dekopon-skylight-seed # plus your existing entries
```

The copied file lands at `<paths.configDir>/private/skylight-refresh-seed`; with the chart's
default `configDir` that is `/etc/dekopon/private/skylight-refresh-seed`.

### 2. Secret map entry

```yaml
- drn: drn:com.xrl:secret:rpi:skylight/oauth2
  source:
    kind: oauth2Refresh
    recordPath: /var/lib/dekopon/oauth2-records/skylight.json
    tokenEndpoint: https://app.ourskylight.com/oauth/token
    clientId: skylight-mobile
    refresh: scheduled
    seedPath: /etc/dekopon/private/skylight-refresh-seed
    timeoutMs: 10000
  projection: {format: utf8}
  bindings:
    - id: skylight-account-read
      capability: skylight-private.account.read
      sink: httpBearer
      allowedHosts: [app.ourskylight.com]
      allowedMethods: [GET]
      allowedPaths: [{match: exact, path: /api/user}]
      maxInjections: 1
    # …one binding per capability, as in the table below
```

`refresh: scheduled` rotates the family when nobody reads Skylight for days; `lazy` (the default)
refreshes only on use, so a long idle gap can outlive the upstream refresh token. Bump `mapRevision` with this change.

Every binding uses `sink: httpBearer`, `allowedHosts: [app.ourskylight.com]`,
`allowedMethods: [GET]` and `maxInjections: 1`:

| Binding ID | Capability | `allowedPaths` | `allowQuery` |
|---|---|---|---|
| `skylight-account-read` | `skylight-private.account.read` | `exact /api/user` | no |
| `skylight-frames-list` | `skylight-private.frames.list` | `exact /api/frames` | no |
| `skylight-categories-list` | `skylight-private.categories.list` | `segmentPrefix /api/frames` | no |
| `skylight-calendar-events-list` | `skylight-private.calendar.events.list` | `segmentPrefix /api/frames` | yes |
| `skylight-lists-list` | `skylight-private.lists.list` | `segmentPrefix /api/frames` | no |
| `skylight-lists-read` | `skylight-private.lists.read` | `segmentPrefix /api/frames` | no |
| `skylight-list-items-list` | `skylight-private.list.items.list` | `segmentPrefix /api/frames` | no |
| `skylight-tasks-list` | `skylight-private.tasks.list` | `segmentPrefix /api/frames` | yes |

Bind only the capabilities you grant. A non-default
[`baseUrl`](../README.md#owner-base-url) host has no binding here and is denied.

### 3. Constraint sets

Start from the README's `capabilities:` example, drop its provider-level `credential:` line, and give
each capability a `defaultSecretUse` in place of `{}`. The constraints stay the same.

```yaml
capabilities:
  skylight-private:
    constraints:
      timeoutMs: 10000
      http:
        allowedHosts: [app.ourskylight.com]
        allowedMethods: [GET]
        maxRequests: 1
        maxRequestBytes: 4096
        maxResponseBytes: 262144
    capabilities:
      skylight-private.account.read:
        defaultSecretUse: {drn: drn:com.xrl:secret:rpi:skylight/oauth2, sink: httpBearer}
      skylight-private.frames.list:
        defaultSecretUse: {drn: drn:com.xrl:secret:rpi:skylight/oauth2, sink: httpBearer}
      # …the same for each granted capability
```

`defaultSecretUse` is not a grant. It fills an omitted proposal before authorization; a missing
binding or a missing `secret.use` permit still denies, and nothing is injected.

### 4. Cedar `secret.use`

A capability permit alone returns `secret-denied`. The DRN needs its own statement:

```cedar
@id("skylight-oauth2-readers")
permit(principal == Dekopon::Principal::"<principal>",
       action == Dekopon::Action::"secret.use",
       resource == Dekopon::Secret::"drn:com.xrl:secret:rpi:skylight/oauth2")
when { context.provider == "skylight-private" && context.sink == "httpBearer"
       && context.agent == "<agent>" };
```

## First seed and reconnect

The record starts from a seed: one fresh Skylight refresh token, captured by the owner out of band.
It is a refresh token, never an access token. This repository has no capture tool, and no agent or
provider ever handles one.

Reconnect when the broker reports `broker.credential.sealed` (at the seal and at every boot while the
record stays sealed), when reads keep failing with a broker failure, or when a live family's reads
keep returning `reauth-required`:

1. Capture a fresh refresh token. Do not log out of the session it came from; abandon it.
2. Change the value of key `refresh-seed` at the Secret's source (for External Secrets, the
   field in the backing store). A hand-written value in a Secret a controller owns is reverted, and
   the reverted, spent seed is adopted at the next rollout and seals the family.
3. Let Reloader roll the pod, or roll it yourself. The chart copies the seed only at pod start.
4. After the roll, a `scheduled` source exchanges the changed seed on the broker's first tick
   (`broker.credential.refresh` with `trigger = scheduled`, `outcome = refreshed`); a `lazy` source
   does it on first use. Either way the broker exchanges it under the record lock, in Live or Sealed
   state, abandoning the old family, and saves the rotation before serving any access token. An
   unchanged seed never unseals or overwrites a rotation. Then make one read-only call such as
   `skylight account`.
5. With `scheduled`, that call's use span reads `outcome = not-due`; with `lazy`, it reads
   `refreshed`.

Delete any local copy of the seed.

A failed save after a rotation has a named cost: after a restart the broker may present the spent
predecessor, Skylight answers with a reused-token code, and the record seals. The recovery is this
same reconnect.

## Reset a malformed record

`broker_credential_record_invalid` means the record is malformed or insecure; the broker refuses that
DRN and keeps serving everything else. Delete the record and provide a new seed. The broker image is
distroless, so follow core's
[maintenance-pod procedure](https://github.com/dekopon-agents/dekopon/blob/v0.48.0/docs/secrets.md#oauth2refresh)
with the broker scaled to zero, but have the pod run `rm /record/skylight.json` instead of writing a
record from stdin.
