# Development

## Setup

```sh
git clone https://github.com/scute-sh/scute.git
cd scute
```

The Rust toolchain is pinned in `crates/rust-toolchain.toml`. Rustup will pick it up automatically when you run cargo commands from within `crates/`.

## Scripts

All operations go through `scripts/scute/`. You don't need to know the underlying tools. Run from the repo root:

```sh
./scripts/scute/fmt.sh          # format code
./scripts/scute/lint.sh         # lint (clippy)
./scripts/scute/test.sh         # run tests
./scripts/scute/ci.sh           # all checks (fmt --check + lint + test)
./scripts/scute/release.sh      # publish + tag + GitHub Release (release-plz)
./scripts/scute/dist.sh         # build distributable binaries for your platform (cargo-dist)
```

Before committing, run `./scripts/scute/ci.sh`. That's the same thing CI runs.

## CI

**Workflow:** `.github/workflows/ci-scute.yml`

Runs on every push to `main` and on PRs that touch `crates/**` or `.scute.yml`. Does one thing: calls `./scripts/scute/ci.sh`.

Changes outside those paths (docs, handbook, etc.) don't trigger CI.

## Releases

**Tools:** [release-plz](https://release-plz.dev) handles versioning, crates.io publishing, and GitHub Releases. [cargo-dist](https://opensource.axo.dev/cargo-dist/) builds cross-platform binaries and pushes the Homebrew formula.

**The flow:**

1. Push conventional commits to `main`
2. release-plz detects releasable changes, opens a Release PR (bumps version in `Cargo.toml`)
3. Review and merge the Release PR
4. release-plz publishes to crates.io, creates a git tag (`v0.2.0`), and creates a draft GitHub Release with generated notes
5. The tag triggers the `release` workflow (cargo-dist)
6. cargo-dist builds binaries for 5 targets, attaches them to the GitHub Release, undrafts it, and pushes the Homebrew formula

**Targets:** `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`

**Installers:** shell (macOS/Linux), powershell (Windows), Homebrew (`scute-sh/homebrew-tap`)

### Emergency release

When CI is down and you need to ship. Requires `CARGO_REGISTRY_TOKEN` in your environment:

```sh
./scripts/scute/release.sh      # publish + tag + GitHub Release
./scripts/scute/dist.sh         # build binaries for your platform
gh release upload v0.x.0 ...    # attach manually
```

## Secrets

| Secret | Purpose |
|--------|---------|
| `RELEASE_APP_ID` | GitHub App ID for release-plz (creates PRs and tags) |
| `RELEASE_APP_PRIVATE_KEY` | GitHub App private key for release-plz |
| `CARGO_REGISTRY_TOKEN` | crates.io publishing (release-plz) |
| `HOMEBREW_TAP_TOKEN` | Push formula to `scute-sh/homebrew-tap` (cargo-dist) |
| `GITHUB_TOKEN` | Built-in, used for PRs and releases |
