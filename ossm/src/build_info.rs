/// Logs build metadata baked in by the firmware's `build.rs`.
///
/// Must be invoked from a firmware crate whose `build.rs` emits
/// `OSSM_DEVICE`, `OSSM_COMMIT_HASH`, `OSSM_RELEASE_ID`, and `OSSM_BUILD_TIME`.
#[macro_export]
macro_rules! build_info {
    () => {
        log::info!(
            "{} {} (release: {}, built: {})",
            env!("OSSM_DEVICE"),
            env!("OSSM_COMMIT_HASH"),
            env!("OSSM_RELEASE_ID"),
            env!("OSSM_BUILD_TIME"),
        )
    };
}
