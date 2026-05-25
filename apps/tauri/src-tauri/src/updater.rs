/// Whether the running build supports the self-updater. Returns:
/// - `true` on macOS, Windows, and Linux AppImage installs.
/// - `false` on Linux `.deb` / `.rpm` installs (system package manager
///   owns updates — running our updater there fights the distro and
///   leaves files orphaned in `/usr/...` outside `apt`/`dnf`'s tracking).
///
/// AppImage detection is per appimage.org's runtime contract: the
/// AppImage runtime sets `APPIMAGE` to the mounted .AppImage's absolute
/// path before launching the embedded binary. `.deb`/`.rpm` users never
/// see this env var, so `is_some()` cleanly separates the two install
/// types without heuristics on `argv[0]` paths.
///
/// docs: https://docs.appimage.org/packaging-guide/environment-variables.html
#[tauri::command]
pub fn updater_supported() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::env::var_os("APPIMAGE").is_some()
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}
