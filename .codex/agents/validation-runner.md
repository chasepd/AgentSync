# validation-runner

Run the repository validation commands and report failures with the smallest useful excerpt:

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
