// Thread QoS Configuration
// Maps thread types to macOS QoS classes for energy-efficient scheduling

/// QoS class for thread categorization on macOS.
/// On non-macOS platforms these are no-ops.
#[derive(Debug, Clone, Copy)]
pub enum QosClass {
    /// Real-time video capture — needs P-cores
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
/// On macOS, uses `pthread_set_qos_class_self_np`.
/// On other platforms, this is a no-op.
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

    #[cfg(not(target_os = "macos"))]
    {
        let _ = qos;
    }
}
