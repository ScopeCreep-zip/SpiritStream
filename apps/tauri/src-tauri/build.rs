fn main() {
    // Forward Cargo's `TARGET` env var into the compiled binary so the
    // runtime can locate dev-mode sidecars at
    // `apps/tauri/src-tauri/binaries/<name>-<TARGET>`. Tauri 2's bundler
    // strips the target-triple suffix in prod, but `tauri dev` doesn't —
    // and `app.shell().sidecar(name)` doesn't expose the resolved path
    // back to the caller, so we recompute it.
    println!(
        "cargo:rustc-env=BUILD_TARGET={}",
        std::env::var("TARGET").unwrap_or_else(|_| "unknown".into())
    );
    tauri_build::build()
}
