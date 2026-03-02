# Babel42 — ROS2 Project Analysis Tool

Babel42 analyzes ROS2 workspaces: package discovery, manifest checks, launch file validation, and more. Use it to find issues in your ROS2 projects and integrate checks into CI.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Crate Structure

- **internals**: Shared library for ROS2 project analysis (package discovery, package.xml parsing, workspace structure)
- **cli**: Open-source CLI with analysis and check commands

## Installation

```bash
git clone https://github.com/elan8/babel42.git
cd babel42
cargo install --path cli
```

## Build and Test

```bash
cargo build
cargo test
```

Unit tests run without fixtures. For integration tests, fetch fixtures first (see [Test Fixtures](#test-fixtures)).

## Usage

```bash
# Analyze a ROS2 workspace
babel42 analyze <path>

# Check for issues (CI-friendly)
babel42 check <path>

# Export project model to JSON
babel42 export <path> --format json
```

Or via cargo run:

```bash
cargo run -p cli -- analyze <path>
cargo run -p cli -- check <path>
```

## CI Integration

Example GitHub Actions workflow:

```yaml
- name: Babel42 check
  run: |
    cargo install --path cli
    babel42 check . --fail-on error
```

This repo includes [.github/workflows/ci.yml](.github/workflows/ci.yml): unit tests and fmt/clippy on every push/PR, integration tests nightly.

## Integration Results

Nightly integration results are available as JSON for website integration:

- **JSON**: [https://elan8.github.io/babel42/latest-results.json](https://elan8.github.io/babel42/latest-results.json)
- **Overview**: [https://elan8.github.io/babel42/](https://elan8.github.io/babel42/)

Fetch example:

```javascript
const res = await fetch('https://elan8.github.io/babel42/latest-results.json');
const data = await res.json();
// data.summary, data.repos, data.total_errors, etc.
```

## Test Fixtures

Fixture repos are gitignored. Clone them with `scripts/fetch_fixtures.ps1` (Windows) or `scripts/fetch_fixtures.sh` (Linux/macOS). See [tests/fixtures/README.md](tests/fixtures/README.md) for the full list and integration test instructions.

## Rules Reference

See [RULES.md](RULES.md) for all checks, severities, and fix hints.
