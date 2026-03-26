# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.12](https://github.com/scute-sh/scute/compare/scute-v0.0.11...scute-v0.0.12) - 2026-03-26

This release focuses on reducing false positives in the code similarity check.
Flat literal lists (reserved slugs, config arrays, `vec!` macros) are no longer
flagged as duplication, and overlapping clone occurrences within the same file
are properly merged instead of reported as separate clones. Tested against a
real-world frontend + backend codebase, these changes eliminated all false
positives while preserving every real duplication finding.

### Fixed

- *(code-similarity)* filter literal-only clones inside collections ([#112](https://github.com/scute-sh/scute/pull/112))
- *(code-similarity)* overlap-based subsumption for clone groups ([#110](https://github.com/scute-sh/scute/pull/110))
- *(code-similarity)* merge overlapping clone occurrences ([#108](https://github.com/scute-sh/scute/pull/108))
- *(code-similarity)* clean up unused imports and add must_use lint

## [0.0.11](https://github.com/scute-sh/scute/compare/scute-v0.0.10...scute-v0.0.11) - 2026-03-20

### Added

- *(code-complexity)* add JavaScript support ([#100](https://github.com/scute-sh/scute/pull/100))

### Other

- *(core)* extract language module, decouple grammar from check rules ([#102](https://github.com/scute-sh/scute/pull/102))

## [0.0.10](https://github.com/scute-sh/scute/compare/scute-v0.0.9...scute-v0.0.10) - 2026-03-20

### Fixed

- *(test-utils)* per-check stdin handling in CliStdin backend ([#99](https://github.com/scute-sh/scute/pull/99))

### Other

- *(test-utils)* typed DSL for all checks, remove string-based API ([#97](https://github.com/scute-sh/scute/pull/97))

## [0.0.9](https://github.com/scute-sh/scute/compare/scute-v0.0.8...scute-v0.0.9) - 2026-03-20

### Added

- *(code-similarity)* TS/JS contract detection ([#91](https://github.com/scute-sh/scute/pull/91))

### Fixed

- *(code-similarity)* harden edge cases and writing style ([#93](https://github.com/scute-sh/scute/pull/93))
- *(code-similarity)* exclude same-contract trait impls ([#86](https://github.com/scute-sh/scute/pull/86))

### Other

- *(test-utils)* typed DSL for commit-message check ([#96](https://github.com/scute-sh/scute/pull/96))
- *(code-similarity)* restructure test pyramid and harden edges ([#95](https://github.com/scute-sh/scute/pull/95))
- *(code-similarity)* redesign pipeline with tree-based architecture ([#89](https://github.com/scute-sh/scute/pull/89))

## [0.0.8](https://github.com/scute-sh/scute/compare/scute-v0.0.7...scute-v0.0.8) - 2026-03-16

### Added

- *(code-complexity)* TypeScript support ([#78](https://github.com/scute-sh/scute/pull/78))

### Fixed

- *(code-complexity)* reject non-this member calls as TS recursion ([#83](https://github.com/scute-sh/scute/pull/83))
- *(code-complexity)* TS scoring gaps and test coverage ([#82](https://github.com/scute-sh/scute/pull/82))

### Other

- *(code-complexity)* decouple scoring engine tests from language ([#81](https://github.com/scute-sh/scute/pull/81))
- *(code-complexity)* clean up check.rs orchestration and tests ([#80](https://github.com/scute-sh/scute/pull/80))
- *(code-complexity)* cognitive roles for Construct ([#76](https://github.com/scute-sh/scute/pull/76))
- *(code-complexity)* language-agnostic scoring engine ([#74](https://github.com/scute-sh/scute/pull/74))

## [0.0.7](https://github.com/scute-sh/scute/compare/scute-v0.0.6...scute-v0.0.7) - 2026-03-15

### Other

- *(test-utils)* add TestDir::source_file ([#72](https://github.com/scute-sh/scute/pull/72))
- eliminate all remaining complexity warnings ([#71](https://github.com/scute-sh/scute/pull/71))
- *(code-similarity)* reduce complexity of collect_test_ranges ([#70](https://github.com/scute-sh/scute/pull/70))
- *(code-similarity)* flatten collect_tokens complexity ([#69](https://github.com/scute-sh/scute/pull/69))
- *(code-complexity)* introduce ScoringContext to reduce complexity and duplication ([#67](https://github.com/scute-sh/scute/pull/67))
- *(report)* replace mutable counters with fold in summarize ([#65](https://github.com/scute-sh/scute/pull/65))
- *(code-similarity)* extract helpers from algorithmic functions ([#63](https://github.com/scute-sh/scute/pull/63))
- *(dependency-freshness)* DRY root detection and location prefixing ([#62](https://github.com/scute-sh/scute/pull/62))
- *(config)* simplify find_config_file with search boundary helper ([#64](https://github.com/scute-sh/scute/pull/64))

## [0.0.6](https://github.com/scute-sh/scute/compare/scute-v0.0.5...scute-v0.0.6) - 2026-03-14

### Added

- *(code-complexity)* actionable evidence with cognitive drivers ([#52](https://github.com/scute-sh/scute/pull/52))
- *(code-complexity)* check for cognitive complexity in Rust functions ([#50](https://github.com/scute-sh/scute/pull/50))

### Fixed

- *(code-complexity)* accept paths directly instead of source-dir + focus files ([#59](https://github.com/scute-sh/scute/pull/59))

### Other

- *(code-complexity)* harden with shared validation and edge cases ([#56](https://github.com/scute-sh/scute/pull/56))
- *(code-complexity)* prove config, exclude, and focus files ([#55](https://github.com/scute-sh/scute/pull/55))
- *(code-complexity)* public documentation ([#57](https://github.com/scute-sh/scute/pull/57))
- *(core)* extract shared tree-sitter parser ([#48](https://github.com/scute-sh/scute/pull/48))

## [0.0.5](https://github.com/scute-sh/scute/compare/scute-v0.0.4...scute-v0.0.5) - 2026-03-12

### Other

- updated the following local packages: scute-core, scute-config, scute-mcp

## [0.0.4](https://github.com/scute-sh/scute/compare/scute-v0.0.3...scute-v0.0.4) - 2026-03-11

### Added

- *(code-similarity)* support file exclude patterns ([#29](https://github.com/scute-sh/scute/pull/29))

## [0.0.3](https://github.com/scute-sh/scute/compare/scute-v0.0.2...scute-v0.0.3) - 2026-03-11

### Other

- updated the following local packages: scute-core, scute-config, scute-mcp

## [0.0.2](https://github.com/scute-sh/scute/compare/scute-v0.0.1...scute-v0.0.2) - 2026-03-09

### Other

- updated the following local packages: scute-core, scute-config, scute-mcp

## [0.0.1](https://github.com/scute-sh/scute/compare/scute-v0.0.0...scute-v0.0.1) - 2026-03-09

### Other

- updated the following local packages: scute-core, scute-mcp, scute-config
