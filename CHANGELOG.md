# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.8.0] - 2026-10-08

### Added

- Add validated owner `providerSettings.skylight-private.baseUrl` routing with unchanged default routes and synthetic cassette replay coverage.

### Changed

- Repin published Dekopon SDK, testkit, and broker test dependencies to 0.38.0.

## [0.7.0] - 2026-10-04

### Changed

- Repin published Dekopon SDK, testkit, and broker dependencies to 0.33.0; component HTTP import uses 1.1.0 with unchanged buffered private reads.

## [0.6.0] - 2026-10-03

### Changed

- Migrate to typed SDK stdio on published Dekopon core 0.31.0; retain fixed private read boundaries and broker credential isolation.
- Use `skylight-private.*` capability IDs and a 1 MiB component artifact ceiling.

## [0.5.0] - 2026-09-20

### Changed

- Move to provider SDK 0.18.0 and HTTP 1.1.0 with its asset type dependency; caller inputs, outputs, grants, and buffered-only HTTP behavior are unchanged.
