// Power Management Service
// Prevents system sleep and App Nap during streaming/recording/capture
// Uses IOKit (IOPMAssertion) for sleep prevention and
// objc2-foundation (NSProcessInfo) for App Nap prevention on macOS

/// Prevents the system from idle-sleeping while held.
/// Automatically releases the assertion on drop.
#[cfg(target_os = "macos")]
pub struct PowerAssertion {
    assertion_id: u32,
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::CString;

    // IOKit power assertion types
    const K_IOPM_ASSERTION_TYPE_NO_IDLE_SLEEP: &str = "NoIdleSleepAssertion";
    const K_IOPM_ASSERTION_TYPE_NO_DISPLAY_SLEEP: &str = "NoDisplaySleepAssertion";
    const K_IOPM_ASSERTION_LEVEL_ON: u32 = 255;

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IOPMAssertionCreateWithName(
            assertion_type: *const std::ffi::c_void,
            assertion_level: u32,
            reason_for_activity: *const std::ffi::c_void,
            assertion_id: *mut u32,
        ) -> i32;
        fn IOPMAssertionRelease(assertion_id: u32) -> i32;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithCString(
            alloc: *const std::ffi::c_void,
            c_str: *const std::ffi::c_char,
            encoding: u32,
        ) -> *const std::ffi::c_void;
        fn CFRelease(cf: *const std::ffi::c_void);
    }

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x08000100;

    fn create_cfstring(s: &str) -> *const std::ffi::c_void {
        let c_str = CString::new(s).unwrap();
        unsafe { CFStringCreateWithCString(std::ptr::null(), c_str.as_ptr(), K_CF_STRING_ENCODING_UTF8) }
    }

    pub fn create_assertion(assertion_type: &str, reason: &str) -> Result<u32, String> {
        let type_cf = create_cfstring(assertion_type);
        let reason_cf = create_cfstring(reason);
        let mut assertion_id: u32 = 0;

        let result = unsafe {
            IOPMAssertionCreateWithName(
                type_cf,
                K_IOPM_ASSERTION_LEVEL_ON,
                reason_cf,
                &mut assertion_id,
            )
        };

        unsafe {
            CFRelease(type_cf);
            CFRelease(reason_cf);
        }

        if result == 0 {
            // kIOReturnSuccess
            Ok(assertion_id)
        } else {
            Err(format!("IOPMAssertionCreate failed with code: {}", result))
        }
    }

    pub fn release_assertion(assertion_id: u32) {
        unsafe {
            IOPMAssertionRelease(assertion_id);
        }
    }

    pub const IDLE_SLEEP_TYPE: &str = K_IOPM_ASSERTION_TYPE_NO_IDLE_SLEEP;
    pub const DISPLAY_SLEEP_TYPE: &str = K_IOPM_ASSERTION_TYPE_NO_DISPLAY_SLEEP;
}

#[cfg(target_os = "macos")]
impl PowerAssertion {
    /// Prevent the system from idle-sleeping (e.g., during streaming/recording).
    /// The assertion is released when this value is dropped.
    pub fn prevent_idle_sleep(reason: &str) -> Result<Self, String> {
        let id = macos::create_assertion(macos::IDLE_SLEEP_TYPE, reason)?;
        log::info!("Acquired idle sleep prevention assertion (id={}): {}", id, reason);
        Ok(Self { assertion_id: id })
    }

    /// Prevent the display from sleeping (e.g., during active streaming).
    /// The assertion is released when this value is dropped.
    pub fn prevent_display_sleep(reason: &str) -> Result<Self, String> {
        let id = macos::create_assertion(macos::DISPLAY_SLEEP_TYPE, reason)?;
        log::info!("Acquired display sleep prevention assertion (id={}): {}", id, reason);
        Ok(Self { assertion_id: id })
    }
}

#[cfg(target_os = "macos")]
impl Drop for PowerAssertion {
    fn drop(&mut self) {
        log::info!("Releasing power assertion (id={})", self.assertion_id);
        macos::release_assertion(self.assertion_id);
    }
}

// Non-macOS: no-op stubs
#[cfg(not(target_os = "macos"))]
pub struct PowerAssertion;

#[cfg(not(target_os = "macos"))]
impl PowerAssertion {
    pub fn prevent_idle_sleep(_reason: &str) -> Result<Self, String> {
        Ok(Self)
    }

    pub fn prevent_display_sleep(_reason: &str) -> Result<Self, String> {
        Ok(Self)
    }
}

/// Prevents macOS App Nap throttling during active capture.
/// Uses NSProcessInfo.beginActivity() / endActivity() via objc2-foundation typed bindings.
/// Automatically ends the activity on drop.
#[cfg(target_os = "macos")]
pub struct ActivityAssertion {
    token: objc2::rc::Retained<objc2::runtime::ProtocolObject<dyn objc2::runtime::NSObjectProtocol>>,
}

#[cfg(target_os = "macos")]
unsafe impl Send for ActivityAssertion {}
#[cfg(target_os = "macos")]
unsafe impl Sync for ActivityAssertion {}

#[cfg(target_os = "macos")]
impl ActivityAssertion {
    /// Begin a user-initiated activity that prevents App Nap.
    /// The activity is ended when this value is dropped.
    pub fn begin(reason: &str) -> Result<Self, String> {
        use objc2_foundation::{NSActivityOptions, NSProcessInfo, NSString};

        let options = NSActivityOptions::UserInitiatedAllowingIdleSystemSleep;
        let process_info = NSProcessInfo::processInfo();
        let reason_ns = NSString::from_str(reason);

        let token = process_info.beginActivityWithOptions_reason(options, &reason_ns);

        log::info!("Acquired App Nap prevention activity: {}", reason);
        Ok(Self { token })
    }
}

#[cfg(target_os = "macos")]
impl Drop for ActivityAssertion {
    fn drop(&mut self) {
        use objc2_foundation::NSProcessInfo;

        let process_info = NSProcessInfo::processInfo();
        // Safety: self.token was created by beginActivityWithOptions_reason
        unsafe {
            process_info.endActivity(&self.token);
        }
        log::info!("Released App Nap prevention activity");
    }
}

// Non-macOS: no-op stubs
#[cfg(not(target_os = "macos"))]
pub struct ActivityAssertion;

#[cfg(not(target_os = "macos"))]
impl ActivityAssertion {
    pub fn begin(_reason: &str) -> Result<Self, String> {
        Ok(Self)
    }
}

/// Check if the system is in Low Power Mode (macOS 12+).
/// Returns `true` if Low Power Mode is active.
/// On non-macOS platforms, always returns `false`.
#[cfg(target_os = "macos")]
pub fn is_low_power_mode() -> bool {
    use objc2_foundation::NSProcessInfo;
    NSProcessInfo::processInfo().isLowPowerModeEnabled()
}

#[cfg(not(target_os = "macos"))]
pub fn is_low_power_mode() -> bool {
    false
}

/// macOS thermal pressure state, matching `NSProcessInfoThermalState`.
///
/// - `Nominal` — No thermal pressure; system operating normally.
/// - `Fair` — Slightly elevated thermal state; system may throttle opportunistic work.
/// - `Serious` — High thermal pressure; system is actively throttling CPU/GPU.
/// - `Critical` — Maximum thermal pressure; performance severely limited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ThermalState {
    Nominal,
    Fair,
    Serious,
    Critical,
}

impl ThermalState {
    /// Convert from the NSInteger value returned by `[NSProcessInfo thermalState]`.
    fn from_ns_integer(value: isize) -> Self {
        match value {
            0 => ThermalState::Nominal,
            1 => ThermalState::Fair,
            2 => ThermalState::Serious,
            3 => ThermalState::Critical,
            _ => {
                log::warn!("Unknown NSProcessInfoThermalState value: {}, defaulting to Nominal", value);
                ThermalState::Nominal
            }
        }
    }
}

/// Query the current macOS thermal pressure state.
///
/// Calls `[NSProcessInfo processInfo].thermalState` via the Objective-C runtime.
/// Returns the current `ThermalState` reflecting the system's thermal pressure.
///
/// On non-macOS platforms, always returns `ThermalState::Nominal`.
#[cfg(target_os = "macos")]
pub fn get_thermal_state() -> ThermalState {
    use objc2::msg_send;
    use objc2_foundation::NSProcessInfo;

    let process_info = NSProcessInfo::processInfo();
    // NSProcessInfo.thermalState returns NSProcessInfoThermalState (NSInteger)
    // 0 = Nominal, 1 = Fair, 2 = Serious, 3 = Critical
    let state: isize = unsafe { msg_send![&process_info, thermalState] };
    ThermalState::from_ns_integer(state)
}

#[cfg(not(target_os = "macos"))]
pub fn get_thermal_state() -> ThermalState {
    ThermalState::Nominal
}

// ============================================================================
// Centralized Power Budget Manager
// ============================================================================
// Ref-counted assertions: multiple services share a single IOKit assertion.
// Only the first acquirer creates it; only the last releaser drops it.
// Polls thermal state every 5s and exposes battery/charging info.

use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;

/// Centralized power and resource manager.
///
/// - Ref-counted `PowerAssertion` and `ActivityAssertion` (one OS assertion shared by N services)
/// - Thermal state monitoring (polled every 5s)
/// - Battery/charging state detection
/// - Emits events when thermal state changes
pub struct PowerBudgetManager {
    /// Ref count for idle-sleep prevention
    power_refs: AtomicUsize,
    /// Ref count for App Nap prevention
    activity_refs: AtomicUsize,
    /// The single shared power assertion (created on first ref, dropped on last)
    power_assertion: Mutex<Option<PowerAssertion>>,
    /// The single shared activity assertion
    activity_assertion: Mutex<Option<ActivityAssertion>>,
    /// Latest cached thermal state
    thermal_state: Mutex<ThermalState>,
    /// Whether thermal monitor thread is running
    monitor_running: AtomicBool,
    /// Shutdown signal for the monitor thread
    shutdown: AtomicBool,
    /// Lock-free throttle flag — set by thermal monitor, checked by encode loops.
    /// Services can share this via `Arc<AtomicBool>` without accessing the full manager.
    throttle_flag: Arc<AtomicBool>,
    /// Event sink for emitting thermal state changes to frontend
    event_sink: Mutex<Option<Arc<dyn crate::services::EventSink>>>,
}

/// Battery and power status
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerStatus {
    pub thermal: ThermalState,
    pub low_power_mode: bool,
    pub power_assertion_refs: usize,
    pub activity_assertion_refs: usize,
}

/// Concrete throttle limits derived from thermal state and CPU architecture.
/// Services query this to decide FPS, quality, and concurrency limits.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThrottlePolicy {
    /// Max preview FPS (normal: 15, throttled: 5)
    pub preview_fps: u32,
    /// Max MJPEG quality (1=best, 31=worst; normal: 5, throttled: 15)
    pub mjpeg_quality: u32,
    /// Max concurrent hardware encoder sessions
    pub max_hw_encoders: u8,
    /// Audio level update interval in ms (normal: 100, throttled: 500)
    pub audio_level_interval_ms: u64,
    /// Whether throttling is active
    pub throttled: bool,
}

impl PowerBudgetManager {
    pub fn new() -> Self {
        // On Intel Macs, start with throttle pre-armed at Fair+ since they're
        // already thermally constrained
        let initial_throttle = is_intel_mac();
        Self {
            power_refs: AtomicUsize::new(0),
            activity_refs: AtomicUsize::new(0),
            power_assertion: Mutex::new(None),
            activity_assertion: Mutex::new(None),
            thermal_state: Mutex::new(ThermalState::Nominal),
            monitor_running: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
            throttle_flag: Arc::new(AtomicBool::new(initial_throttle)),
            event_sink: Mutex::new(None),
        }
    }

    /// Acquire a power assertion ref. Creates the OS assertion on first ref.
    pub fn acquire_power(&self, reason: &str) {
        let prev = self.power_refs.fetch_add(1, Ordering::SeqCst);
        if prev == 0 {
            let mut guard = self.power_assertion.lock();
            if guard.is_none() {
                match PowerAssertion::prevent_idle_sleep(reason) {
                    Ok(assertion) => {
                        log::info!("PowerBudget: Created shared idle-sleep assertion: {}", reason);
                        *guard = Some(assertion);
                    }
                    Err(e) => {
                        log::warn!("PowerBudget: Failed to create power assertion: {}", e);
                        self.power_refs.fetch_sub(1, Ordering::SeqCst);
                    }
                }
            }
        } else {
            log::debug!("PowerBudget: Power ref acquired (count={})", prev + 1);
        }
    }

    /// Release a power assertion ref. Drops the OS assertion on last release.
    pub fn release_power(&self) {
        let prev = self.power_refs.fetch_sub(1, Ordering::SeqCst);
        if prev == 1 {
            let mut guard = self.power_assertion.lock();
            if guard.is_some() {
                log::info!("PowerBudget: Releasing shared idle-sleep assertion (last ref)");
                *guard = None; // Drop triggers IOPMAssertionRelease
            }
        } else if prev == 0 {
            // Underflow protection
            self.power_refs.store(0, Ordering::SeqCst);
            log::warn!("PowerBudget: Power ref underflow detected");
        } else {
            log::debug!("PowerBudget: Power ref released (count={})", prev - 1);
        }
    }

    /// Acquire an activity assertion ref. Creates the OS assertion on first ref.
    pub fn acquire_activity(&self, reason: &str) {
        let prev = self.activity_refs.fetch_add(1, Ordering::SeqCst);
        if prev == 0 {
            let mut guard = self.activity_assertion.lock();
            if guard.is_none() {
                match ActivityAssertion::begin(reason) {
                    Ok(assertion) => {
                        log::info!("PowerBudget: Created shared activity assertion: {}", reason);
                        *guard = Some(assertion);
                    }
                    Err(e) => {
                        log::warn!("PowerBudget: Failed to create activity assertion: {}", e);
                        self.activity_refs.fetch_sub(1, Ordering::SeqCst);
                    }
                }
            }
        } else {
            log::debug!("PowerBudget: Activity ref acquired (count={})", prev + 1);
        }
    }

    /// Release an activity assertion ref. Drops the OS assertion on last release.
    pub fn release_activity(&self) {
        let prev = self.activity_refs.fetch_sub(1, Ordering::SeqCst);
        if prev == 1 {
            let mut guard = self.activity_assertion.lock();
            if guard.is_some() {
                log::info!("PowerBudget: Releasing shared activity assertion (last ref)");
                *guard = None;
            }
        } else if prev == 0 {
            self.activity_refs.store(0, Ordering::SeqCst);
            log::warn!("PowerBudget: Activity ref underflow detected");
        } else {
            log::debug!("PowerBudget: Activity ref released (count={})", prev - 1);
        }
    }

    /// Start the thermal monitoring thread (polls every 5s).
    /// Safe to call multiple times — only starts once.
    pub fn start_thermal_monitor(self: &Arc<Self>) {
        if self.monitor_running.swap(true, Ordering::SeqCst) {
            return; // Already running
        }

        let this = Arc::clone(self);
        std::thread::Builder::new()
            .name("ss-thermal-monitor".to_string())
            .spawn(move || {
                log::info!("PowerBudget: Thermal monitor started");
                while !this.shutdown.load(Ordering::Relaxed) {
                    let new_state = get_thermal_state();
                    let old_state = {
                        let mut guard = this.thermal_state.lock();
                        let old = *guard;
                        *guard = new_state;
                        old
                    };

                    if old_state != new_state {
                        log::info!(
                            "PowerBudget: Thermal state changed: {:?} -> {:?}",
                            old_state, new_state
                        );

                        // Update lock-free throttle flag based on thermal state + architecture
                        let should_throttle = match new_state {
                            ThermalState::Serious | ThermalState::Critical => true,
                            ThermalState::Fair => is_intel_mac(),
                            ThermalState::Nominal => false,
                        };
                        this.throttle_flag.store(should_throttle, Ordering::Relaxed);

                        // Emit WebSocket event to frontend
                        if let Some(ref sink) = *this.event_sink.lock() {
                            sink.emit("thermal_state_changed", serde_json::json!({
                                "state": format!("{:?}", new_state),
                                "previousState": format!("{:?}", old_state),
                                "throttled": should_throttle,
                                "policy": this.throttle_policy(),
                            }));
                        }

                        if new_state == ThermalState::Serious || new_state == ThermalState::Critical {
                            log::warn!(
                                "PowerBudget: High thermal pressure ({:?}) — throttling active",
                                new_state
                            );
                        }
                    }

                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
                log::info!("PowerBudget: Thermal monitor stopped");
            })
            .ok();
    }

    /// Get the current thermal state (cached, updated every 5s).
    pub fn thermal_state(&self) -> ThermalState {
        *self.thermal_state.lock()
    }

    /// Whether thermal state suggests we should throttle (Serious or Critical,
    /// or Fair+ on Intel Macs).
    pub fn should_throttle(&self) -> bool {
        self.throttle_flag.load(Ordering::Relaxed)
    }

    /// Get a shared reference to the throttle flag for encode loops.
    /// This avoids passing the full PowerBudgetManager into every thread.
    pub fn throttle_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.throttle_flag)
    }

    /// Get concrete throttle limits based on thermal state and architecture.
    /// Intel Macs get more aggressive throttling even at lower thermal states.
    pub fn throttle_policy(&self) -> ThrottlePolicy {
        let state = *self.thermal_state.lock();
        let intel = is_intel_mac();

        match state {
            ThermalState::Nominal if !intel => ThrottlePolicy {
                preview_fps: 15,
                mjpeg_quality: 5,
                max_hw_encoders: 3,
                audio_level_interval_ms: 100,
                throttled: false,
            },
            ThermalState::Nominal if intel => ThrottlePolicy {
                preview_fps: 15,
                mjpeg_quality: 8,
                max_hw_encoders: 1,
                audio_level_interval_ms: 100,
                throttled: false,
            },
            ThermalState::Fair => ThrottlePolicy {
                preview_fps: if intel { 8 } else { 12 },
                mjpeg_quality: if intel { 12 } else { 8 },
                max_hw_encoders: if intel { 1 } else { 2 },
                audio_level_interval_ms: if intel { 200 } else { 100 },
                throttled: intel,
            },
            ThermalState::Serious => ThrottlePolicy {
                preview_fps: 5,
                mjpeg_quality: 15,
                max_hw_encoders: 1,
                audio_level_interval_ms: 300,
                throttled: true,
            },
            ThermalState::Critical => ThrottlePolicy {
                preview_fps: 2,
                mjpeg_quality: 20,
                max_hw_encoders: 0,
                audio_level_interval_ms: 500,
                throttled: true,
            },
            // Catch-all for Nominal on Intel (the `if intel` guard above)
            _ => ThrottlePolicy {
                preview_fps: 15,
                mjpeg_quality: 5,
                max_hw_encoders: 3,
                audio_level_interval_ms: 100,
                throttled: false,
            },
        }
    }

    /// Get full power status for the API endpoint.
    pub fn status(&self) -> PowerStatus {
        PowerStatus {
            thermal: *self.thermal_state.lock(),
            low_power_mode: is_low_power_mode(),
            power_assertion_refs: self.power_refs.load(Ordering::Relaxed),
            activity_assertion_refs: self.activity_refs.load(Ordering::Relaxed),
        }
    }

    /// Set the event sink for thermal state change notifications.
    /// Call after EventBus is created in main.rs.
    pub fn set_event_sink(&self, sink: Arc<dyn crate::services::EventSink>) {
        *self.event_sink.lock() = Some(sink);
    }

    /// Shut down the thermal monitor.
    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

impl Drop for PowerBudgetManager {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        // Assertions are dropped automatically via their Option wrappers
    }
}

/// Returns true if running on Apple Silicon (aarch64 macOS).
/// Intel Macs (x86_64) lack efficiency cores and have weaker thermal
/// dissipation, requiring more aggressive CPU budget management.
pub fn is_apple_silicon() -> bool {
    cfg!(all(target_os = "macos", target_arch = "aarch64"))
}

/// Returns true if running on Intel Mac (x86_64 macOS).
pub fn is_intel_mac() -> bool {
    cfg!(all(target_os = "macos", target_arch = "x86_64"))
}

impl Default for PowerBudgetManager {
    fn default() -> Self {
        Self::new()
    }
}
