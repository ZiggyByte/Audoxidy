# Contributing to Audoxidy

Thank you for your interest in contributing to **Audoxidy**! This guide will help you get started with the development workflow, code conventions, and the process for submitting changes.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Development Setup](#development-setup)
- [Project Overview](#project-overview)
- [Code Style](#code-style)
- [Commit Conventions](#commit-conventions)
- [Testing](#testing)
- [Pull Request Process](#pull-request-process)
- [Issue Reporting](#issue-reporting)
- [License](#license)

## Code of Conduct

- Be respectful and constructive in all interactions
- Focus on the technical problem, not the person
- Welcome newcomers and help them get started
- Give credit where it's due

## Development Setup

### Prerequisites

- **Rust** — stable toolchain with edition 2024 support (1.98.1+)
- **Linux**: `libasound2-dev` (ALSA) and optionally `libpipewire-0.3-dev` (PipeWire)
- **Windows/macOS**: no additional system dependencies

### Getting Started

```bash
# 1. Fork and clone
git clone https://github.com/<your-username>/Audoxidy.git
cd Audoxidy

# 2. Build
cargo build --release

# 3. Run tests
cargo test

# 4. Run the application
cargo run --release
```

### Recommended Tools

- [cargo-nextest](https://github.com/nextest-rs/nextest) — faster test runner (2-3x speedup)
- [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) — code coverage
- [sccache](https://github.com/mozilla/sccache) — compilation cache
- [flamegraph](https://github.com/flamegraph-rs/flamegraph) — performance profiling

## Project Overview

Audoxidy is a single Rust binary crate with the following source layout:

| Directory | Purpose |
|-----------|---------|
| `src/audio/` | Audio engine, decoder, DSP chain, device management |
| `src/db/` | SQLite library, scanner, metadata extraction |
| `src/gui/` | Iced 0.14 UI — player, playlist, library, audio center, widgets |
| `src/utils/` | Config, cover art cache, string interner, memory manager |
| `src/integrations/` | MPRIS/SMTC media controls, PipeWire/PulseAudio config |

### Key Architecture Notes

- **GUI thread** communicates with the **audio decode thread** via a crossbeam channel
- **AudioState** and **DspChain** use `parking_lot::RwLock` for concurrent access
- The **Database** uses `std::sync::Mutex` (shared between GUI and audio threads)
- Never hold an `RwLock` read guard while acquiring a write guard on the same lock — this causes deadlocks

## Code Style

### Formatting and Linting

Always run before committing:

```bash
cargo fmt
cargo clippy
```

### Rust Conventions

- Follow standard Rust idioms and naming conventions
- Use `snake_case` for functions and variables, `PascalCase` for types
- Prefer `#[derive(Default)]` or manual `Default` impl with carefully chosen values
- Use `.expect("descriptive message")` for critical startup paths, `.unwrap()` in tests
- Use `Result<T, AudioError>` for audio module functions — never return `String` errors

### Import Organization

1. Standard library (`std::*`)
2. External crates (`cpal`, `symphonia`, `iced`, etc.)
3. Internal modules (`crate::audio::*`, `crate::db::*`, etc.)

### Comments

- Use `//` for implementation comments
- Use `// ── Section ──` dividers for logical grouping
- Use `// D-XX:` references for design decisions
- Comments may be in Spanish (for domain logic) or English (for code structure)

## Commit Conventions

We use [Conventional Commits](https://www.conventionalcommits.org/) for clear, structured history:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

### Types

| Type | When to use |
|------|-------------|
| `feat` | New feature or capability |
| `fix` | Bug fix |
| `refactor` | Code restructuring without behavior change |
| `test` | Adding or updating tests |
| `docs` | Documentation changes |
| `chore` | Build, dependencies, CI, or tooling |
| `perf` | Performance improvement |

### Scopes

| Scope | Area |
|-------|------|
| `audio` | Audio engine, decoder, DSP |
| `gui` | Iced UI, widgets, views |
| `db` | Database, scanner |
| `integrations` | MPRIS/SMTC, PipeWire |
| `utils` | Config, covers, memory |

### Examples

```
feat(audio): add EQ preset import/export via JSON
fix(gui): prevent playlist scroll jump on track change
refactor(db): batch song inserts for faster scanning
test(audio): add crossfade edge case tests
chore: update dependencies — fix yanked lofty
```

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run with nextest (faster)
cargo nextest run

# Run specific module tests
cargo test audio::tests
cargo test audio::dsp_tests

# Run with output
cargo test -- --nocapture
```

### Test Conventions

- Audio engine tests: `src/audio/tests.rs`
- DSP tests: `src/audio/dsp_tests.rs`
- Widget tests: `src/gui/widgets_tests.rs`
- Inline tests use `#[cfg(test)]` modules within source files
- Use `assert!((a - b).abs() < epsilon)` for float comparisons — never `==`
- Test helpers are `pub(crate)` when shared across test modules

### Writing Good Tests

- Test both happy path and error cases
- Use descriptive test names: `test_compressor_threshold_activated`
- Keep tests independent — no shared mutable state between tests
- Use `Database::new_memory()` for database tests (in-memory SQLite)

## Pull Request Process

### Before Submitting

1. **Ensure it compiles**: `cargo check`
2. **Run tests**: `cargo test`
3. **Format code**: `cargo fmt`
4. **Lint**: `cargo clippy`
5. **Rebase** on latest `main` if needed

### PR Description

Include in your PR:

- **What** changed and **why**
- **How** it was implemented (brief overview)
- **Screenshots** for UI changes
- **Test coverage** — what tests were added/updated

### Review Checklist

- [ ] Code compiles without warnings
- [ ] All existing tests pass
- [ ] New code has corresponding tests
- [ ] `cargo fmt` and `cargo clippy` are clean
- [ ] No secrets or credentials are committed
- [ ] Commit messages follow conventional format

### After Review

- Address feedback promptly
- Force-push amended commits (don't create fixup commits)
- The reviewer will merge once approved

## Issue Reporting

### Bug Reports

Include:

- **Steps to reproduce**
- **Expected behavior**
- **Actual behavior**
- **Audio format** and **sample rate** of the file (if audio-related)
- **OS and audio backend** (ALSA, PipeWire, WASAPI, CoreAudio)
- **Log output** from `~/.local/share/audoxidy/logs/` (if available)

### Feature Requests

Include:

- **Use case** — what problem does this solve?
- **Proposed behavior** — what should happen?
- **Alternatives considered** — have you thought of other approaches?

## License

By contributing, you agree that your contributions will be licensed under the **GNU General Public License v3.0**.
