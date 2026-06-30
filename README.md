# Decimate

![Decimate banner](assets/decimate-banner.png)

Find dead Dart code, circular dependencies, duplicated code, complex functions,
dependency problems, risky Flutter wiring, and PR risk fast.

![Rust 1.85+](https://img.shields.io/badge/rust-1.85%2B-b7410e)
![Dart and Flutter](https://img.shields.io/badge/Dart%20%2B%20Flutter-codebase%20intelligence-111111)
![License](https://img.shields.io/badge/license-MIT-2f855a)

Decimate is a Rust-native codebase intelligence tool for Dart and Flutter. It
looks at your repo as a graph:

- Dart files are nodes.
- `import`, `export`, `part`, `part of`, and `library augment` are edges.
- The report tells you what is unused, risky, duplicated, tangled, or hard to
  maintain.

It is not a formatter. It is not a replacement for `dart analyze`. It is not a
Flutter style guide. It does not enforce opinions like "all providers must use
Riverpod code generation."

It answers practical questions:

- What code can probably be deleted?
- What files depend on each other in a circle?
- What functions are too complex?
- What code was copied around?
- What dependency is unused or missing from `pubspec.yaml`?
- What changed code is risky before a PR lands?
- What should an AI coding agent inspect before making a fix?

## Start Here

Inside any Dart or Flutter project:

```bash
decimate check . --format json --summary | jq .summary
```

If you do not want JSON:

```bash
decimate check .
```

If the report says `"verdict": "fail"`, Decimate worked. It means it found
error-level issues. It does not mean the tool crashed.

Exit codes:

- `0`: no error-level findings
- `1`: Decimate found issues
- `2`: command, config, or runtime error
- `8`: security gate found new review-required candidates

## Install

From GitHub:

```bash
cargo install --git https://github.com/sgaabdu4/decimate
```

Cargo installs `decimate` and `decimate-mcp` into `~/.cargo/bin`.

Fish:

```bash
fish_add_path ~/.cargo/bin
```

Bash or Zsh:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

From a local checkout:

```bash
git clone https://github.com/sgaabdu4/decimate.git
cd decimate
cargo install --path . --force
```

Then run it in your app:

```bash
cd /path/to/flutter_or_dart_repo
decimate check . --format json
```

## npx

The npm package name is `@sgaabdu4/decimate`.

After the package is published:

```bash
npx @sgaabdu4/decimate check . --format json
```

Before publication, npm will return `404 Not Found`. Use Cargo or:

```bash
npx --yes --package github:sgaabdu4/decimate decimate check . --format json
```

## What Decimate Looks For

### 1. Dead Code

Dead code is code that is not reachable from your entry points.

Decimate finds:

- dead Dart files
- unused public exports
- unused type aliases
- unused enum values
- unused private class members
- unrendered Flutter widgets
- missing entry points
- stale `decimate-ignore` / `fallow-ignore` comments

Useful commands:

```bash
decimate dead-code . --entry lib/main.dart --format json
decimate check . --unused-files --unused-exports --unused-deps --format json
```

### 2. Complex Code

Cyclomatic complexity means "how many paths can this function take?"

Cognitive complexity means "how hard is this function to understand?"

CRAP score combines complexity with test coverage. A complex function with poor
coverage gets a worse score.

Decimate finds:

- high cyclomatic complexity
- high cognitive complexity
- high combined complexity
- high CRAP score
- coverage gaps
- low health score files
- hotspots
- refactoring targets

Useful commands:

```bash
decimate health . --format json
decimate health . --complexity-breakdown --top 10 --format json
decimate health . --file-scores --hotspots --targets --format json
```

### 3. Duplicated Code

Duplication means the same Dart code appears in more than one place.

Decimate finds exact and semantic clone groups. Each clone group gets a stable
fingerprint like `dup:abc12345`, so agents can trace it before touching code.

Useful commands:

```bash
decimate dupes . --format json
decimate dupes . --mode semantic --min-lines 5 --format json
decimate dupes . --threshold 5 --format json
decimate trace-clone . --fingerprint dup:abc12345 --format json
```

### 4. Circular Dependencies

A circular dependency means file A depends on file B, and B eventually depends
back on A. Cycles make code harder to move, test, and delete.

Decimate finds:

- circular dependencies
- re-export cycles
- import/export/part/augment targets that do not resolve
- invalid `part` / `part of` relationships

Useful commands:

```bash
decimate cycles . --format json
decimate check . --circular-deps --re-export-cycles --format json
```

### 5. Architecture Drift

Architecture drift means code crosses boundaries it should not cross.

Example: `lib/domain/` depending on `lib/ui/`.

Decimate finds:

- boundary violations
- files outside configured boundary zones
- forbidden direct calls across boundaries
- policy-pack violations for banned imports, exports, and calls

Useful command:

```bash
decimate check . \
  --boundary lib/domain:lib/ui \
  --boundary-coverage \
  --format json
```

### 6. Dependency Hygiene

Dependency hygiene means your imports and `pubspec.yaml` agree.

Decimate finds:

- unused runtime dependencies
- unused dev dependencies
- runtime dependencies used only by tests
- imports missing from `pubspec.yaml`
- unused dependency overrides
- invalid dependency overrides
- imports into another package's private `lib/src`
- duplicate public API exports

Useful commands:

```bash
decimate trace-dependency . --dependency collection --format json
decimate check . --unused-deps --unlisted-deps --private-src-imports --format json
```

### 7. Flutter-Specific Graph Issues

Decimate does not care which state-management style you use. It uses Flutter
and Dart patterns only to avoid false positives and to find graph problems.

Decimate finds:

- GoRouter route path/name collisions
- private Flutter widget classes
- top-level widget helper functions
- unused widget constructor parameters
- widget classes that are never constructed
- missing `context.mounted` guards after awaited widget work

### 8. Security Candidates

These are review prompts, not proof of an exploit.

Decimate finds candidates for:

- hardcoded secrets
- insecure HTTP transport
- TLS validation bypasses
- risky WebView settings
- process execution
- raw SQL
- plain local storage of secret-like material

Useful commands:

```bash
decimate security . --surface --format json
decimate security . --ci --sarif-file decimate-security.sarif
git diff --cached --unified=0 | decimate security . --gate new --diff-stdin --format json
```

### 9. PR Risk

Use this before merging changed code.

Decimate reports:

- risk score
- pass / warn / fail risk level
- findings introduced by the PR
- findings that already existed
- risky changed files

Useful commands:

```bash
decimate audit . --base origin/main --format json
decimate audit . --base origin/main --gate new-only --format json
```

### 10. Runtime Intelligence

Static analysis says what is connected. Runtime coverage says what actually ran.

Decimate can read LCOV, V8, and Istanbul coverage data.

Useful commands:

```bash
decimate health . --coverage coverage/lcov.info --coverage-gaps --max-crap 30 --format json
decimate coverage analyze . --runtime-coverage coverage-final.json --format json
```

## How To Read The Summary

Example:

```json
{
  "files": 466,
  "edges": 1231,
  "quality_score": 93,
  "cycles": 2,
  "code_duplications": 26,
  "complex_functions": 13,
  "dead_files": 11,
  "findings": 125
}
```

Plain English:

- `files`: Dart files Decimate parsed
- `edges`: imports, exports, parts, and augments it resolved
- `quality_score`: project health from `0` to `100`
- `cycles`: circular dependency groups
- `code_duplications`: duplicated code groups
- `complex_functions`: functions over the complexity limits
- `dead_files`: files Decimate thinks are unreachable
- `findings`: total issues in the report

## JSON For Agents

Use JSON when another tool or AI agent will read the result:

```bash
decimate check . --format json
```

Every finding includes:

- `rule_id`
- `kind`
- `severity`
- `path`
- `line`
- `column`
- `safe_to_delete`
- related `files`
- suggested `actions`

Example shape:

```json
{
  "schema_version": "decimate.report.v1",
  "kind": "combined",
  "tool": "decimate",
  "command": "check",
  "verdict": "fail",
  "summary": {
    "files": 466,
    "edges": 1231,
    "quality_score": 93,
    "findings": 125
  },
  "findings": [],
  "next_steps": []
}
```

If a JSON command fails before a report can be built, stdout still stays
machine-readable:

```json
{ "error": true, "message": "coverage analyze requires --runtime-coverage PATH", "exit_code": 2 }
```

## Fixes

Preview safe fixes:

```bash
decimate fix . --format json
```

Apply confirmed safe fixes:

```bash
decimate fix . --apply --confirm --format json
```

Safe fixes are intentionally conservative. Decimate can currently apply:

- simple dead-file deletion
- stale suppression removal
- one-line unused Pub dependency removal
- one-line unused top-level Dart declaration removal

Everything else stays review-only.

## Watch Mode

Rerun checks while you work:

```bash
decimate watch . --no-clear
```

Run once and exit, useful for scripts:

```bash
decimate watch . --once --format json
```

## MCP

Start the MCP server:

```bash
decimate-mcp
```

Agents can use it to inspect a project, trace files, trace symbols, inspect
duplicates, review PR risk, read runtime coverage slices, preview fixes, and
ask what is safest to do next.

`fix_apply` is the only mutating MCP tool and requires explicit `yes: true`.

## Config

Decimate reads config from:

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
threshold = 5

[boundaries]
presets = ["layered"]
rules = ["lib/domain:lib/ui"]

[security]
surface = true
categories = ["hardcoded-secret", "insecure-transport", "tls-bypass"]

[rules]
unused-files = "error"
unused-exports = "warn"
security-candidate = "warn"
```

## Full Issue List

Run this for the live list:

```bash
decimate schema --format json | jq .issue_types
```

Current issue types:

```text
dead-file
unused-export
unused-type
private-type-leak
unused-enum-member
unused-class-member
duplicate-export
route-collision
private-widget-class
widget-top-level-function-boundary
unused-widget-param
unrendered-widget
missing-context-mounted-after-await
missing-entry-point
circular-dependency
re-export-cycle
boundary-violation
boundary-coverage
boundary-call-violation
policy-violation
unresolved-dependency
part-of-violation
unused-dependency
unused-dev-dependency
test-only-dependency
unused-dependency-override
misconfigured-dependency-override
unlisted-dependency
private-src-import
code-duplication
high-cyclomatic-complexity
high-cognitive-complexity
high-complexity
coverage-gap
high-crap-score
health-hotspot
refactoring-target
feature-flag
security-candidate
stale-suppression
missing-suppression-reason
```

## CI

Typical GitHub Actions command:

```bash
decimate audit . --base origin/main --format json --fail-on-regression
```

Generate CI templates:

```bash
decimate ci-template github --format yaml
decimate ci-template gitlab --format yaml
```

Preview review-thread reconciliation without changing GitHub or GitLab:

```bash
decimate ci reconcile-review \
  --provider github \
  --repo owner/repo \
  --pr 123 \
  --envelope review-github.json \
  --dry-run \
  --format json
```

## Scope

Decimate implements local Fallow-style codebase intelligence for Dart and
Flutter.

Fallow features that are JS-specific or require hosted backends return clear
unsupported JSON instead of pretending to work:

```bash
decimate migrate --dry-run --format json
decimate telemetry status --format json
decimate license status --format json
```

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
npm run pack:check
```

This repository forbids `unsafe_code`.

## License

Licensed under the MIT License. See [LICENSE](LICENSE).
