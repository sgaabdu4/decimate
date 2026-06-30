# Decimate

Rust-native codebase intelligence for Dart and Flutter module graphs.

![Rust 1.85+](https://img.shields.io/badge/rust-1.85%2B-b7410e)
![Dart and Flutter](https://img.shields.io/badge/Dart%20%2B%20Flutter-module%20graph-0175c2)
![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-2f855a)

Decimate is a fast, deterministic analyzer for Dart and Flutter codebases. It is
inspired by Fallow's agent-first approach to codebase intelligence, adapted to
Dart's library, `part`, `export`, `package:`, and Pub workspace semantics.

It is not a linter, formatter, type checker, or runtime debugger. Decimate
treats a project as a graph:

- files are nodes
- `import`, `export`, `part`, and `library augment` directives are edges
- declarations, members, feature flags, security candidates, duplication, and
  health metrics become structured evidence for humans and agents

The goal is simple: make cleanup, architectural review, and AI-assisted code
maintenance cheap enough to run constantly.

## What Decimate Finds

Decimate currently reports:

- unreachable Dart files
- unused public top-level declarations and type aliases
- unused enum constants and private class-like members
- private Dart library types leaking through public signatures
- duplicate public exports
- typed and raw GoRouter route path and name collisions
- circular dependencies and export-only barrel cycles
- invalid `part` / `part of` relationships
- unresolved local imports, exports, parts, and augmentations
- architecture boundary violations, boundary coverage gaps, and forbidden calls
- declarative policy-pack violations
- unused, unlisted, misplaced, and override-related Pub dependencies
- duplicated Dart code blocks with stable `dup:<id>` fingerprints
- cyclomatic, cognitive, CRAP, coverage-gap, hotspot, ownership, and
  refactoring-target health signals
- feature flag usage in Dart compile-time environment reads, `Platform`
  environment gates, Firebase Remote Config, and LaunchDarkly-style SDK calls
- manual Riverpod provider declarations that should move to generated
  `@riverpod` owners
- Flutter widget classes that are never constructed from reachable production
  code
- local, unverified Dart and Flutter security review candidates
- stale suppressions and missing suppression reasons

Every JSON finding includes file paths, line numbers, severity, `safe_to_delete`,
and machine-actionable `actions`.

## Quick Start

Build from source:

```bash
git clone https://github.com/sgaabdu4/decimate.git
cd decimate
cargo build --release
```

Run a full static check:

```bash
./target/release/decimate
./target/release/decimate check . --format json
```

Install from a local checkout:

```bash
cargo install --path .
decimate check . --format json
```

Install from GitHub after cloning is not required:

```bash
cargo install --git https://github.com/sgaabdu4/decimate
```

Run through npm/npx:

```bash
npx @sgaabdu4/decimate check . --format json
npx @sgaabdu4/decimate check --root . --format json
npx --package @sgaabdu4/decimate decimate check . --format json
```

The npm package exposes the executable as `decimate`. The unscoped npm package
name `decimate` is already owned by an unrelated GeoJSON package, so
`npx decimate` cannot safely target this project unless that package name is
transferred. Public npm installs use `@sgaabdu4/decimate`.

Root-aware commands accept both the existing positional `ROOT` form
(`decimate check .`) and the Fallow-style `--root ROOT` form
(`decimate check --root .`).

Exit code `0` means no error-severity findings. Exit code `1` means findings
were produced. Exit code `2` means command/config/runtime failure. Security gate
mode can exit `8` for new review-required security candidates.

When `--format json` is present and Decimate cannot produce the requested report,
stdout is still machine-readable:

```json
{ "error": true, "message": "coverage analyze requires --runtime-coverage PATH", "exit_code": 2 }
```

## Common Workflows

Initialize Decimate defaults in a Dart or Flutter repo:

```bash
decimate init . --agents
```

`decimate init` writes `.decimaterc` and, with `--agents`, an `AGENTS.md`
guide for downstream coding agents. It refuses to overwrite existing files
unless `--force` is passed.

Install a managed Git pre-commit hook:

```bash
decimate hooks status . --format json
decimate hooks install . --target git --branch origin/main --format json
decimate hooks uninstall . --target git --format json
```

Hook install refuses to overwrite non-Decimate hooks unless `--force` is passed,
and uninstall only removes hooks containing Decimate's ownership marker.

Find dead code:

```bash
decimate dead-code . --entry lib/main.dart --format json
decimate dead-code . --unused-exports --format json
decimate check . --unused-files --unused-deps --format json
```

Issue filters follow Fallow naming where Decimate has real Dart data:
`--unused-files`, `--unused-exports`, `--unused-types`, `--unused-deps`,
`--unlisted-deps`, `--duplicate-exports`, `--unused-enum-members`,
`--unused-class-members`, `--unresolved-imports`, `--stale-suppressions`,
`--unused-dependency-overrides`, and `--misconfigured-dependency-overrides`.

Find cycles:

```bash
decimate cycles . --format json
```

Find duplicate Dart code:

```bash
decimate dupes . --format json
decimate dupes . --mode semantic --format json
decimate dupes . --ignore-imports --format json
```

Review changed code against `origin/main`:

```bash
decimate audit . --base origin/main --format json
```

Surface architecture decisions without failing CI:

```bash
decimate review . --base origin/main --format json
decimate decision-surface . --base origin/main --format json
```

Prioritize refactors:

```bash
decimate health . --file-scores --hotspots --targets --ownership --format json
```

Add LCOV or runtime coverage context:

```bash
decimate coverage setup . --non-interactive --format json
decimate coverage setup . --yes --format json
decimate health . --coverage coverage/lcov.info --coverage-gaps --max-crap 30 --format json
decimate coverage analyze . --runtime-coverage coverage-final.json --format json
decimate coverage upload-inventory . --dry-run --repo owner/repo --format json
decimate coverage upload-source-maps . --dir dist --git-sha <sha> --repo owner/repo --dry-run --format json
```

Inventory feature flags:

```bash
decimate flags . --format json
decimate flags . --top 20 --format json
```

Surface security candidates for verification:

```bash
decimate security . --surface --format json
decimate security . --ci --sarif-file decimate-security.sarif
git diff --cached --unified=0 | decimate security . --gate new --diff-stdin --format json
```

Trace before deleting:

```bash
decimate trace-file . --file lib/src/old.dart --format json
decimate trace --root . lib/src/old.dart:OldThing --format json
decimate trace-symbol . --symbol lib/src/old.dart:OldThing --format json
decimate trace-dependency . --dependency collection --format json
decimate trace-clone . --fingerprint dup:abc12345 --format json
```

Preview and apply safe fixes:

```bash
decimate fix . --format json
decimate fix . --apply --confirm --format json
```

Safe fixes are intentionally conservative. Decimate can currently apply:

- dead Dart file deletion when graph evidence marks the file safe
- stale suppression removal
- simple one-line unused Pub dependency removal
- simple one-line unused top-level Dart declaration removal

Multi-line declarations, public API barrels, members, security candidates, and
complex Pub dependency forms stay review-only.

## Agent JSON Contract

Use `--format json` for machine-readable output.

The main report envelope is `decimate.report.v1`:

```json
{
  "schema_version": "decimate.report.v1",
  "kind": "combined",
  "tool": "decimate",
  "command": "check",
  "verdict": "fail",
  "summary": { "files": 42, "findings": 3 },
  "findings": [],
  "next_steps": []
}
```

Discover the full command and issue manifest:

```bash
decimate schema --format json
decimate report-schema --format json
decimate config-schema --format json
decimate rule-pack-schema --format json
```

Important schemas:

- `decimate.report.v1`: analysis reports
- `decimate.trace.v1`: trace reports
- `decimate.inspect.v1`: file/symbol evidence bundles
- `decimate.fix.v1`: safe-fix preview/apply reports
- `decimate.init.v1`: project initialization reports
- `decimate.hooks.v1`: hook status/install/uninstall reports
- `decimate.decision-surface.v1`: changed-code review questions
- `decimate.coverage.v1`: runtime coverage setup, analysis, and dry-run upload packets
- `decimate.ci-template.v1`: CI template output

## Configuration

Decimate discovers config from:

1. `.decimaterc`
2. `.decimaterc.json`
3. `.decimaterc.jsonc`
4. `decimate.toml`
5. `.decimate.toml`

Example:

```toml
[cli]
format = "json"
entry = ["lib/main.dart"]
production = true

[health]
max_cyclomatic = 20
max_cognitive = 15
coverage_gaps = true
fileScores = true
hotspots = true
targets = true

[dupes]
mode = "semantic"
min_tokens = 80

[security]
surface = true
categories = ["hardcoded-secret", "insecure-transport", "tls-bypass"]

ignore_patterns = ["**/*.g.dart", "**/*.freezed.dart"]
ignore_dependencies = ["build_runner"]

[rules]
unused-files = "error"
unused-exports = "warn"
security-candidate = "warn"
```

Architecture boundaries can be passed on the CLI:

```bash
decimate check . \
  --boundary lib/domain:lib/ui \
  --boundary-coverage \
  --boundary-call 'lib/domain:FirebaseRemoteConfig.*' \
  --format json
```

Policy packs are pure JSON/JSONC data. They can ban import/export URI patterns
or direct call patterns without executing project code.

## CI

Generate CI templates:

```bash
decimate ci-template github --format yaml
decimate ci-template gitlab --format yaml
```

Typical GitHub Actions command:

```bash
decimate audit . --base origin/main --format json --fail-on-regression
```

For security code scanning:

```bash
decimate security . --ci --sarif-file decimate-security.sarif
```

## Fallow Parity Status

Decimate mirrors Fallow's core static intelligence workflow for Dart and
Flutter:

- bare `decimate` defaults to the full combined static check
- agent-first JSON reports, actions, schemas, and next steps
- `decimate init --agents` onboarding for config and agent guidance
- `decimate hooks install --target git` pre-commit audit hook management
- dead code, unused exports/types/members, and dependency hygiene
- cycles, re-export cycles, boundaries, policy packs, and suppressions
- duplication detection with traceable fingerprints
- health, complexity, CRAP, coverage gaps, hotspots, ownership, and targets
- Flutter typed and raw GoRouter route-collision checks
- private Flutter widget class visibility checks
- top-level Flutter widget helper boundary checks
- unused Flutter widget constructor parameter checks
- manual Riverpod provider wiring checks
- unrendered Flutter widget class checks
- feature flag inventory
- local security candidates with SARIF, surface inventory, and changed-code gates
- changed-code audit and advisory decision-surface review
- safe fix previews and confirmed apply flows
- local impact reporting
- local runtime coverage setup, ingestion, inventory dry-runs, and source-map
  upload dry-runs

Decimate also adds Dart-specific graph intelligence that Fallow does not need:

- `part` and `part of` relationship validation
- `library augment` dependency edges
- conditional import/export branch scanning
- Dart library privacy rules
- Pub `.dart_tool/package_config.json`, path dependency, and workspace
  resolution
- `pubspec.yaml`, `dev_dependencies`, `dependency_overrides`, and
  `pubspec.lock` hygiene

Known gaps before claiming full product parity with Fallow:

- no MCP/server API yet
- no embedded Node/NAPI-style bindings, because Decimate is not a JS tool
- no hosted/cloud continuous runtime monitoring
- no `watch`, `migrate`, telemetry, license, editor, or viz commands yet
- hook parity is Git-only; no managed agent hook target yet
- coverage upload commands are intentionally offline dry-runs; real hosted
  source-map/inventory uploads and `coverage analyze --cloud` are not enabled
  yet
- broader Flutter-framework intelligence is still partial: deeper Riverpod
  dependency semantics and richer widget lifecycle heuristics are not complete
- feature flags are inventory-focused and do not yet model owner, expiry, stale
  rollout state, or runtime stale-flag evidence as richly as Fallow
- security candidates are Dart/Flutter-focused and configurable by category, but
  Decimate does not yet expose Fallow's broader request-receiver/catalog model
- symbol auto-fix is intentionally limited to one-line top-level declarations

## Security Model

`decimate security` surfaces deterministic local candidates for review. It does
not prove exploitability and it is not a replacement for CodeQL, Semgrep, Snyk,
OSV, or dependency CVE scanning.

Security output is designed for downstream verification:

- source evidence is redacted
- candidates include category, sink, confidence, path, line, and fingerprint
- `--surface` adds attack-surface prompts
- SARIF output works with code scanning
- `--gate new` and `--gate newly-reachable` reduce pre-commit/CI noise

## Development

Use the local Rust toolchain or any Rust 1.85+ installation.

```bash
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

This repository forbids `unsafe_code`.

## License

Licensed under either MIT or Apache-2.0.
