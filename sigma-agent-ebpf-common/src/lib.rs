#![no_std]

/// Key for the per-PID traffic BPF map.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TrafficKey {
    pub pid: u32,
}

/// Accumulated byte counters per PID.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TrafficValue {
    pub bytes_sent: u64,
    pub bytes_recv: u64,
}

/// TCP retransmit counter per PID.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RetransmitValue {
    pub count: u64,
}

/// TCP connection counters per PID.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ConnValue {
    pub active: u64,
    pub total: u64,
}
