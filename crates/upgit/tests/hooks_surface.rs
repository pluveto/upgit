//! Tracked git hooks must run the same fmt/clippy gates as CI.

const PRE_COMMIT: &str = include_str!("../../../.githooks/pre-commit");
const PRE_PUSH: &str = include_str!("../../../.githooks/pre-push");

#[test]
fn pre_commit_runs_fmt_and_clippy() {
    assert!(PRE_COMMIT.contains("cargo fmt --all -- --check"));
    assert!(PRE_COMMIT.contains("cargo clippy --workspace --all-targets --locked"));
    assert!(PRE_COMMIT.contains("-D warnings"));
}

#[test]
fn pre_push_runs_fmt_and_clippy() {
    assert!(PRE_PUSH.contains("cargo fmt --all -- --check"));
    assert!(PRE_PUSH.contains("cargo clippy --workspace --all-targets --locked"));
    assert!(PRE_PUSH.contains("-D warnings"));
}
