# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.5](https://github.com/scute-sh/scute/compare/scute-core-v0.0.4...scute-core-v0.0.5) - 2026-03-12

### Added

- *(dependency-freshness)* add pnpm workspace support ([#43](https://github.com/scute-sh/scute/pull/43))

### Other

- *(dependency-freshness)* identify JS package managers by lock file ([#45](https://github.com/scute-sh/scute/pull/45))

## [0.0.4](https://github.com/scute-sh/scute/compare/scute-core-v0.0.3...scute-core-v0.0.4) - 2026-03-11

### Added

- *(dependency-freshness)* polyglot monorepo support ([#36](https://github.com/scute-sh/scute/pull/36))
- *(dependency-freshness)* add npm workspace support ([#35](https://github.com/scute-sh/scute/pull/35))
- *(dependency-freshness)* add npm support for single projects ([#32](https://github.com/scute-sh/scute/pull/32))
- *(code-similarity)* support file exclude patterns ([#29](https://github.com/scute-sh/scute/pull/29))

### Other

- *(dependency-freshness)* hardening pass ([#39](https://github.com/scute-sh/scute/pull/39))
- *(dependency-freshness)* restructure tests along the test pyramid ([#38](https://github.com/scute-sh/scute/pull/38))
- *(dependency-freshness)* replace PackageManager enum with trait ([#37](https://github.com/scute-sh/scute/pull/37))
- *(dependency-freshness)* reorganize module structure ([#34](https://github.com/scute-sh/scute/pull/34))

## [0.0.3](https://github.com/scute-sh/scute/compare/scute-core-v0.0.2...scute-core-v0.0.3) - 2026-03-11

### Added

- *(code-similarity)* support JavaScript and JSX files ([#21](https://github.com/scute-sh/scute/pull/21))

### Fixed

- *(code-similarity)* parse .tsx files with the TSX grammar ([#20](https://github.com/scute-sh/scute/pull/20))

## [0.0.2](https://github.com/scute-sh/scute/compare/scute-core-v0.0.1...scute-core-v0.0.2) - 2026-03-09

### Fixed

- *(ci)* narrow release trigger to CLI crate and drop OpenMP dep ([#14](https://github.com/scute-sh/scute/pull/14))

## [0.0.1](https://github.com/scute-sh/scute/compare/scute-core-v0.0.0...scute-core-v0.0.1) - 2026-03-09

### Fixed

- show per-occurrence snippets in code similarity evidence ([#13](https://github.com/scute-sh/scute/pull/13))

### Other

- deduplicate e2e clone detection tests ([#12](https://github.com/scute-sh/scute/pull/12))
- deduplicate tokenizer snapshot tests with macro ([#7](https://github.com/scute-sh/scute/pull/7))
