// Thread QoS Configuration
// Maps thread types to platform-specific priority classes for energy-efficient scheduling.
// - macOS: QoS classes via pthread_set_qos_class_self_np (P-core / E-core scheduling)
// - Windows: Win32 thread priority via SetThreadPriority
// - Linux: nice values via setpriority

/// QoS class for thread categorization.
/// Mapped to platform-specific priority mechanisms:
/// - macOS: QoS classes (UserInteractive → P-cores, Background → E-cores)
/// - Windows: Thread priority levels (TIME_CRITICAL → LOWEST)
/// - Linux: nice values (-20 → 19)
#[derive(Debug, Clone, Copy)]
pub enum QosClass {
    /// Real-time video capture — needs P-cores / highest priority
    UserInteractive,
    /// Latency-sensitive encoding, visible to user
    UserInitiated,
    /// Default scheduling (audio level polling at 10Hz)
    Default,
    /// Background I/O (FFmpeg stderr reader, device discovery)
    Utility,
    /// Minimal CPU, E-cores only (health checks, monitors)
    Background,
}

/// Set the QoS class for the current thread.
///
/// Platform behavior:
/// - **macOS**: Uses `pthread_set_qos_class_self_np` to set QoS class.
/// - **Windows**: Uses `SetThreadPriority` to set thread priority level.
/// - **Linux**: Uses `setpriority` to set the nice value (requires CAP_SYS_NICE for negative values).
/// - **Other**: No-op.
pub fn set_thread_qos(qos: QosClass) {
    #[cfg(target_os = "macos")]
    {
        // macOS QoS class constants from <sys/qos.h>
        const QOS_CLASS_USER_INTERACTIVE: u32 = 0x21;
        const QOS_CLASS_USER_INITIATED: u32 = 0x19;
        const QOS_CLASS_DEFAULT: u32 = 0x15;
        const QOS_CLASS_UTILITY: u32 = 0x11;
        const QOS_CLASS_BACKGROUND: u32 = 0x09;

        let qos_value = match qos {
            QosClass::UserInteractive => QOS_CLASS_USER_INTERACTIVE,
            QosClass::UserInitiated => QOS_CLASS_USER_INITIATED,
            QosClass::Default => QOS_CLASS_DEFAULT,
            QosClass::Utility => QOS_CLASS_UTILITY,
            QosClass::Background => QOS_CLASS_BACKGROUND,
        };

        extern "C" {
            fn pthread_set_qos_class_self_np(qos_class: u32, relative_priority: i32) -> i32;
        }

        let ret = unsafe { pthread_set_qos_class_self_np(qos_value, 0) };
        if ret != 0 {
            log::debug!("pthread_set_qos_class_self_np({:?}) returned {}", qos, ret);
        }
    }

    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
            THREAD_PRIORITY_HIGHEST, THREAD_PRIORITY_LOWEST, THREAD_PRIORITY_NORMAL,
            THREAD_PRIORITY_TIME_CRITICAL,
        };

        let priority = match qos {
            QosClass::UserInteractive => THREAD_PRIORITY_TIME_CRITICAL,
            QosClass::UserInitiated => THREAD_PRIORITY_HIGHEST,
            QosClass::Default => THREAD_PRIORITY_NORMAL,
            QosClass::Utility => THREAD_PRIORITY_BELOW_NORMAL,
            QosClass::Background => THREAD_PRIORITY_LOWEST,
        };

        let ret = unsafe { SetThreadPriority(GetCurrentThread(), priority) };
        if ret == 0 {
            log::debug!("SetThreadPriority({:?}) failed", qos);
        }
    }

    #[cfg(target_os = "linux")]
    {
        let nice = match qos {
            QosClass::UserInteractive => -20,
            QosClass::UserInitiated => -10,
            QosClass::Default => 0,
            QosClass::Utility => 10,
            QosClass::Background => 19,
        };

        // setpriority returns -1 on error; negative nice values require CAP_SYS_NICE
        let ret = unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, nice) };
        if ret != 0 {
            log::debug!(
                "setpriority({:?}, nice={}) failed: {}",
                qos,
                nice,
                std::io::Error::last_os_error()
            );
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = qos;
    }
}

/// Set maximum thread priority for real-time audio processing.
/// - macOS: UserInteractive QoS (P-cores, highest priority)
/// - Windows: THREAD_PRIORITY_TIME_CRITICAL
/// - Linux: nice -20 (requires CAP_SYS_NICE or root)
pub fn set_max_priority() {
    set_thread_qos(QosClass::UserInteractive);
    log::debug!("Audio mixer thread priority set to max (UserInteractive QoS)");
}

/// Set Mach real-time thread scheduling policy for audio threads.
///
/// Uses `thread_policy_set(THREAD_TIME_CONSTRAINT_POLICY)` which tells the
/// macOS scheduler exactly how much CPU time the thread needs per period.
/// This is what CoreAudio and Firefox/WebRTC use for audio threads.
///
/// Falls back to QoS UserInteractive on failure (non-fatal).
///
/// # Arguments
/// * `buffer_frames` - Audio buffer size in frames (e.g., 512, 1024)
/// * `sample_rate` - Sample rate in Hz (e.g., 48000)
#[cfg(target_os = "macos")]
pub fn set_realtime_audio_priority(buffer_frames: u32, sample_rate: u32) {
    use std::mem;

    // Mach time base info — converts nanoseconds to Mach absolute time units
    #[repr(C)]
    struct MachTimebaseInfo {
        numer: u32,
        denom: u32,
    }

    #[repr(C)]
    struct ThreadTimeConstraintPolicy {
        period: u32,
        computation: u32,
        constraint: u32,
        preemptible: i32,
    }

    extern "C" {
        fn mach_timebase_info(info: *mut MachTimebaseInfo) -> i32;
        fn pthread_self() -> usize;
        fn pthread_mach_thread_np(thread: usize) -> u32;
        fn thread_policy_set(
            thread: u32,
            flavor: u32,
            policy_info: *const ThreadTimeConstraintPolicy,
            count: u32,
        ) -> i32;
    }

    const THREAD_TIME_CONSTRAINT_POLICY: u32 = 2;
    const THREAD_TIME_CONSTRAINT_POLICY_COUNT: u32 =
        (mem::size_of::<ThreadTimeConstraintPolicy>() / mem::size_of::<u32>()) as u32;

    // Get Mach timebase for ns → absolute time conversion
    let mut timebase = MachTimebaseInfo { numer: 0, denom: 0 };
    let ret = unsafe { mach_timebase_info(&mut timebase) };
    if ret != 0 || timebase.denom == 0 {
        log::warn!("mach_timebase_info failed (ret={}), falling back to QoS", ret);
        set_max_priority();
        return;
    }

    // Convert nanoseconds to Mach absolute time units
    let ns_to_abs = |ns: u64| -> u32 {
        ((ns * timebase.denom as u64) / timebase.numer as u64) as u32
    };

    // Period = time between audio callbacks (buffer_frames / sample_rate)
    let period_ns = (buffer_frames as u64 * 1_000_000_000) / sample_rate as u64;
    // Computation = how much CPU we need (50% of period is conservative)
    let computation_ns = period_ns / 2;
    // Constraint = hard deadline (90% of period)
    let constraint_ns = period_ns * 9 / 10;

    let policy = ThreadTimeConstraintPolicy {
        period: ns_to_abs(period_ns),
        computation: ns_to_abs(computation_ns),
        constraint: ns_to_abs(constraint_ns),
        preemptible: 1, // Can be preempted (standard for audio)
    };

    let thread_port = unsafe { pthread_mach_thread_np(pthread_self()) };
    let ret = unsafe {
        thread_policy_set(
            thread_port,
            THREAD_TIME_CONSTRAINT_POLICY,
            &policy,
            THREAD_TIME_CONSTRAINT_POLICY_COUNT,
        )
    };

    if ret != 0 {
        log::warn!(
            "thread_policy_set(TIME_CONSTRAINT) failed (ret={}), falling back to QoS",
            ret
        );
        set_max_priority();
    } else {
        log::info!(
            "Audio mixer thread set to Mach real-time (period={}us, computation={}us, constraint={}us)",
            period_ns / 1000, computation_ns / 1000, constraint_ns / 1000
        );
    }
}

/// Non-macOS fallback — uses QoS max priority
#[cfg(not(target_os = "macos"))]
pub fn set_realtime_audio_priority(_buffer_frames: u32, _sample_rate: u32) {
    set_max_priority();
}
