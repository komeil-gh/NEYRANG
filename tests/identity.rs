use neyrang::engine::info::{ENGINE_AUTHOR, ENGINE_NAME, ENGINE_VERSION};

#[test]
fn engine_identity_is_centralized_and_stable() {
    assert_eq!(ENGINE_NAME, "NEYRANG");
    assert_eq!(ENGINE_VERSION, env!("CARGO_PKG_VERSION"));
    assert_eq!(ENGINE_AUTHOR, "NEYRANG Project");
}
