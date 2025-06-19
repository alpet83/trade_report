# Trade Report

Rust-based server for trade reports, risk monitoring, and auditing.

## Setup
1. Install Rust: https://rustup.rs
2. Install vcpkg and dependencies (cairo, pango):
   - `git clone https://github.com/microsoft/vcpkg`
   - `.\vcpkg\bootstrap-vcpkg.bat`
   - `vcpkg install cairo pango`
3. Install MySQL client (mysql-connector-c).
4. Run `cargo build`.

## Development
- Use VS Code with `rust-analyzer` and `CodeLLDB` extensions.
- Configure debugging in `.vscode/launch.json`.
- Cross-compile for Linux: `cross build --target x86_64-unknown-linux-musl`.

## Deployment
- Target: Linux (e.g., Ubuntu).
- Install system libraries: `libmysqlclient-dev`, `libcairo2-dev`, `libpango1.0-dev`.
- Use Docker for MySQL/ClickHouse and application.