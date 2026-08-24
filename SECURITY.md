# Security policy

## Unsupported exploration

This repository is an **exploration only—opt-in, unofficial, private, unsupported, mock-only, not
production** provider. It is not affiliated with or endorsed by Skylight. Do not deploy it from a
default catalog, image, policy, credential set, package, or deployment.

The component has no OAuth, login, refresh, enrollment, endpoint override, or direct network path.
It imports only Dekopon's broker-owned HTTP interface. Credentials must remain disposable,
short-lived static bearers in the owner-only broker store, destination-bound to
`app.ourskylight.com`, and named without PII. Never submit a credential, captured private response,
account ID, frame ID, frame name, or other household metadata in an issue or test fixture.

## Reporting

Use GitHub's private security-advisory reporting for this repository once it exists publicly. Until
then, report through the Dekopon maintainers' existing private security channel. Do not demonstrate
a report against Skylight or any public host; use only synthetic in-memory data.

There are no supported versions and no production service-level commitment. Security fixes are
forward-only. If the fixed private routes or broker contract cannot be kept within the documented
boundary, the provider should remain unavailable rather than add broader authority.
