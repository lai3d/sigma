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

/// TCP RTT (round-trip time) statistics per PID.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RttValue {
    pub count: u64,
    pub sum_us: u64,
    pub min_us: u64,
    pub max_us: u64,
}

/// TCP connection latency statistics per PID (SYN-to-established time).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ConnLatencyValue {
    pub count: u64,
    pub sum_us: u64,
    pub min_us: u64,
    pub max_us: u64,
}

/// Composite key for per-PID per-reason packet drop counters.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DropKey {
    pub pid: u32,
    pub reason: u32,
}

/// Packet drop counter value.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DropValue {
    pub count: u64,
}

/// DNS query counters per PID (UDP sends to port 53).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DnsQueryValue {
    pub queries: u64,
    pub bytes: u64,
}

/// Exec event counter per PID (sched:sched_process_exec tracepoint).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ExecValue {
    pub count: u64,
}
