# Contributing

Thank you for helping improve Agent-Aware Zellij Tab Bar. The project favors
small, composable changes with stable interfaces over broad feature additions.

## Scope

Contributions should help visualize or report agent state in Zellij. Agent
orchestration, prompts, LLM integrations, terminal emulation, Git operations,
and cloud services are intentionally out of scope.

Discuss a large feature or protocol change before implementing it. Architecture
changes should update `ARCHITECTURE.md` in the same pull request.

## Prerequisites

- Rust stable, compatible with Rust 1.92 or newer
- `rustup` with the `wasm32-wasip1` target
- Zellij for manual plugin testing
- Optional quality-gate tools: `cargo-nextest`, `cargo-llvm-cov`,
  `cargo-deny`, and `cargo-audit`

The checked-in `rust-toolchain.toml` installs the Rust components and WASM target
used by the workspace.

## Set Up

```console
git clone <repository-url>
cd agent-tab-bar
cargo build --workspace
./scripts/build.sh
```

`scripts/build.sh` creates the release plugin at
`plugins/zellij-agent-tab-bar.wasm` and the native CLI at
`target/release/zja`. It also verifies the WASM exports required by Zellij,
including the WASI `_start` entry point.

## Design Rules

- Keep dependencies one-way: `cli -> shared <- plugin`.
- Route state changes through immutable domain events.
- Key state by stable tab and pane IDs, never display position.
- Preserve backward compatibility for the CLI, configuration, and JSON
  protocol.
- Prefer private concrete implementations and small substitution seams.
- Do not use `unwrap()` or `expect()` outside tests.
- Treat messages, configuration, paths, and host state as untrusted input.
- Keep updates event-driven; do not add polling for convenience.
- Add dependencies only when the maintenance and binary-size cost is justified.

## Tests

Start with the narrowest test that exercises the change:

```console
cargo test --package zellij-agent-shared
cargo test --workspace
```

Changes to reducers and formatting should include focused unit tests. State
machine changes should include property tests when arbitrary event ordering can
expose invariants. User-visible lifecycle changes should add or update an
integration test.

Tests must cover error paths without relying on production panics.

## Quality Gates

Run the repository check script before opening a pull request:

```console
./scripts/check.sh
```

It runs:

```console
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo nextest run --workspace
cargo llvm-cov --workspace
cargo deny check
cargo audit
```

The last four commands require their corresponding Cargo subcommands. If a
failure is unrelated to a change, report it rather than hiding it.

## Documentation

Update the nearest public documentation whenever behavior changes:

- CLI usage: `README.md`
- configuration: `docs/configuration.md`
- wire compatibility: `docs/protocol.md`
- architectural boundaries: `ARCHITECTURE.md`
- user-visible changes: `CHANGELOG.md`

Examples must remain copyable and must not contain machine-specific paths,
credentials, or terminal output from private sessions.

## Pull Requests

Keep pull requests focused. Include:

- the problem and chosen approach;
- compatibility or protocol impact;
- tests performed;
- manual Zellij verification, when applicable; and
- performance or dependency impact, when applicable.

Do not include generated binaries such as the plugin WASM unless a release
process explicitly requires them.

Security-sensitive findings must follow `SECURITY.md` instead of a public issue.
