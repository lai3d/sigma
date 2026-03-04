#![no_std]
#![no_main]

use aya_ebpf::{
    helpers::{bpf_get_current_pid_tgid, bpf_probe_read_kernel},
    macros::{kprobe, kretprobe, map},
    maps::HashMap,
    programs::{ProbeContext, RetProbeContext},
};
use sigma_agent_ebpf_common::{ConnValue, RetransmitValue, RttValue, TrafficKey, TrafficValue};

/// Offset of `srtt_us` field within `struct tcp_sock` (Linux 6.x x86_64).
/// The kernel stores smoothed RTT as `actual_rtt_us << 3`, so we right-shift by 3 to unscale.
/// This offset varies by kernel version/config — rebuild if targeting a different kernel.
const SRTT_US_OFFSET: usize = 744;

/// Per-PID TCP traffic counters. Userspace reads and periodically clears this map.
#[map]
static TRAFFIC_MAP: HashMap<TrafficKey, TrafficValue> = HashMap::with_max_entries(8192, 0);

/// Per-PID UDP traffic counters. Separate map from TCP for distinguishable metrics.
#[map]
static UDP_TRAFFIC_MAP: HashMap<TrafficKey, TrafficValue> = HashMap::with_max_entries(8192, 0);

/// Per-PID TCP retransmit counters.
#[map]
static RETRANSMIT_MAP: HashMap<TrafficKey, RetransmitValue> = HashMap::with_max_entries(8192, 0);

/// Per-PID TCP connection counters (active gauge + total cumulative).
#[map]
static CONN_MAP: HashMap<TrafficKey, ConnValue> = HashMap::with_max_entries(8192, 0);

/// Per-PID TCP RTT statistics (count, sum, min, max in microseconds).
#[map]
static RTT_MAP: HashMap<TrafficKey, RttValue> = HashMap::with_max_entries(8192, 0);

/// kprobe on tcp_sendmsg — called as tcp_sendmsg(struct sock *sk, struct msghdr *msg, size_t size)
/// The third argument (size) is the number of bytes being sent.
#[kprobe]
pub fn tcp_sendmsg(ctx: ProbeContext) -> u32 {
    match try_tcp_sendmsg(&ctx) {
        Ok(()) => 0,
        Err(_) => 0,
    }
}

fn try_tcp_sendmsg(ctx: &ProbeContext) -> Result<(), i64> {
    let size: u64 = ctx.arg(2).ok_or(1i64)?;
    let pid = (bpf_get_current_pid_tgid() >> 32) as u32;

    let key = TrafficKey { pid };

    // Try to update existing entry, otherwise insert new
    if let Some(val) = TRAFFIC_MAP.get_ptr_mut(&key) {
        unsafe {
            (*val).bytes_sent += size;
        }
    } else {
        let val = TrafficValue {
            bytes_sent: size,
            bytes_recv: 0,
        };
        let _ = TRAFFIC_MAP.insert(&key, &val, 0);
    }

    Ok(())
}

/// kretprobe on tcp_recvmsg — the return value is the number of bytes received (or negative on error).
#[kretprobe]
pub fn tcp_recvmsg(ctx: RetProbeContext) -> u32 {
    match try_tcp_recvmsg(&ctx) {
        Ok(()) => 0,
        Err(_) => 0,
    }
}

fn try_tcp_recvmsg(ctx: &RetProbeContext) -> Result<(), i64> {
    let ret: i64 = ctx.ret().ok_or(1i64)?;
    if ret <= 0 {
        return Ok(()); // Error or zero-length recv, skip
    }
    let size = ret as u64;
    let pid = (bpf_get_current_pid_tgid() >> 32) as u32;

    let key = TrafficKey { pid };

    if let Some(val) = TRAFFIC_MAP.get_ptr_mut(&key) {
        unsafe {
            (*val).bytes_recv += size;
        }
    } else {
        let val = TrafficValue {
            bytes_sent: 0,
            bytes_recv: size,
        };
        let _ = TRAFFIC_MAP.insert(&key, &val, 0);
    }

    Ok(())
}

/// kprobe on udp_sendmsg — called as udp_sendmsg(struct sock *sk, struct msghdr *msg, size_t size)
/// The third argument (size) is the number of bytes being sent.
#[kprobe]
pub fn udp_sendmsg(ctx: ProbeContext) -> u32 {
    match try_udp_sendmsg(&ctx) {
        Ok(()) => 0,
        Err(_) => 0,
    }
}

fn try_udp_sendmsg(ctx: &ProbeContext) -> Result<(), i64> {
    let size: u64 = ctx.arg(2).ok_or(1i64)?;
    let pid = (bpf_get_current_pid_tgid() >> 32) as u32;

    let key = TrafficKey { pid };

    if let Some(val) = UDP_TRAFFIC_MAP.get_ptr_mut(&key) {
        unsafe {
            (*val).bytes_sent += size;
        }
    } else {
        let val = TrafficValue {
            bytes_sent: size,
            bytes_recv: 0,
        };
        let _ = UDP_TRAFFIC_MAP.insert(&key, &val, 0);
    }

    Ok(())
}

/// kretprobe on udp_recvmsg — the return value is the number of bytes received (or negative on error).
#[kretprobe]
pub fn udp_recvmsg(ctx: RetProbeContext) -> u32 {
    match try_udp_recvmsg(&ctx) {
        Ok(()) => 0,
        Err(_) => 0,
    }
}

fn try_udp_recvmsg(ctx: &RetProbeContext) -> Result<(), i64> {
    let ret: i64 = ctx.ret().ok_or(1i64)?;
    if ret <= 0 {
        return Ok(()); // Error or zero-length recv, skip
    }
    let size = ret as u64;
    let pid = (bpf_get_current_pid_tgid() >> 32) as u32;

    let key = TrafficKey { pid };

    if let Some(val) = UDP_TRAFFIC_MAP.get_ptr_mut(&key) {
        unsafe {
            (*val).bytes_recv += size;
        }
    } else {
        let val = TrafficValue {
            bytes_sent: 0,
            bytes_recv: size,
        };
        let _ = UDP_TRAFFIC_MAP.insert(&key, &val, 0);
    }

    Ok(())
}

/// kprobe on tcp_retransmit_skb — called on each TCP retransmit event.
#[kprobe]
pub fn tcp_retransmit_skb(ctx: ProbeContext) -> u32 {
    match try_tcp_retransmit_skb(&ctx) {
        Ok(()) => 0,
        Err(_) => 0,
    }
}

fn try_tcp_retransmit_skb(_ctx: &ProbeContext) -> Result<(), i64> {
    let pid = (bpf_get_current_pid_tgid() >> 32) as u32;
    let key = TrafficKey { pid };

    if let Some(val) = RETRANSMIT_MAP.get_ptr_mut(&key) {
        unsafe {
            (*val).count += 1;
        }
    } else {
        let val = RetransmitValue { count: 1 };
        let _ = RETRANSMIT_MAP.insert(&key, &val, 0);
    }

    Ok(())
}

/// kretprobe on tcp_v4_connect — on success (ret==0), increments active and total connections.
#[kretprobe]
pub fn tcp_v4_connect(ctx: RetProbeContext) -> u32 {
    match try_tcp_v4_connect(&ctx) {
        Ok(()) => 0,
        Err(_) => 0,
    }
}

fn try_tcp_v4_connect(ctx: &RetProbeContext) -> Result<(), i64> {
    let ret: i64 = ctx.ret().ok_or(1i64)?;
    if ret != 0 {
        return Ok(()); // Connection failed, skip
    }

    let pid = (bpf_get_current_pid_tgid() >> 32) as u32;
    let key = TrafficKey { pid };

    if let Some(val) = CONN_MAP.get_ptr_mut(&key) {
        unsafe {
            (*val).active += 1;
            (*val).total += 1;
        }
    } else {
        let val = ConnValue { active: 1, total: 1 };
        let _ = CONN_MAP.insert(&key, &val, 0);
    }

    Ok(())
}

/// kprobe on tcp_close — decrements active connections (saturating).
#[kprobe]
pub fn tcp_close(ctx: ProbeContext) -> u32 {
    match try_tcp_close(&ctx) {
        Ok(()) => 0,
        Err(_) => 0,
    }
}

fn try_tcp_close(_ctx: &ProbeContext) -> Result<(), i64> {
    let pid = (bpf_get_current_pid_tgid() >> 32) as u32;
    let key = TrafficKey { pid };

    if let Some(val) = CONN_MAP.get_ptr_mut(&key) {
        unsafe {
            if (*val).active > 0 {
                (*val).active -= 1;
            }
        }
    }

    Ok(())
}

/// kretprobe on inet_csk_accept — incoming connection accepted (return non-null).
#[kretprobe]
pub fn inet_csk_accept(ctx: RetProbeContext) -> u32 {
    match try_inet_csk_accept(&ctx) {
        Ok(()) => 0,
        Err(_) => 0,
    }
}

fn try_inet_csk_accept(ctx: &RetProbeContext) -> Result<(), i64> {
    let ret: u64 = ctx.ret().ok_or(1i64)?;
    if ret == 0 {
        return Ok(()); // NULL return — accept failed
    }

    let pid = (bpf_get_current_pid_tgid() >> 32) as u32;
    let key = TrafficKey { pid };

    if let Some(val) = CONN_MAP.get_ptr_mut(&key) {
        unsafe {
            (*val).active += 1;
            (*val).total += 1;
        }
    } else {
        let val = ConnValue { active: 1, total: 1 };
        let _ = CONN_MAP.insert(&key, &val, 0);
    }

    Ok(())
}

/// kprobe on tcp_rcv_established — called as tcp_rcv_established(struct sock *sk, ...)
/// Reads srtt_us from the tcp_sock struct to track per-process RTT.
#[kprobe]
pub fn tcp_rcv_established(ctx: ProbeContext) -> u32 {
    match try_tcp_rcv_established(&ctx) {
        Ok(()) => 0,
        Err(_) => 0,
    }
}

fn try_tcp_rcv_established(ctx: &ProbeContext) -> Result<(), i64> {
    let sk: *const u8 = ctx.arg(0).ok_or(1i64)?;
    if sk.is_null() {
        return Ok(());
    }

    // Read srtt_us from tcp_sock at the known offset.
    // The kernel stores it as actual_rtt << 3, so right-shift to get microseconds.
    let srtt_raw: u32 = unsafe {
        bpf_probe_read_kernel(sk.add(SRTT_US_OFFSET) as *const u32).map_err(|_| 1i64)?
    };
    let rtt_us = (srtt_raw >> 3) as u64;

    if rtt_us == 0 {
        return Ok(());
    }

    let pid = (bpf_get_current_pid_tgid() >> 32) as u32;
    let key = TrafficKey { pid };

    if let Some(val) = RTT_MAP.get_ptr_mut(&key) {
        unsafe {
            (*val).count += 1;
            (*val).sum_us += rtt_us;
            if rtt_us < (*val).min_us {
                (*val).min_us = rtt_us;
            }
            if rtt_us > (*val).max_us {
                (*val).max_us = rtt_us;
            }
        }
    } else {
        let val = RttValue {
            count: 1,
            sum_us: rtt_us,
            min_us: rtt_us,
            max_us: rtt_us,
        };
        let _ = RTT_MAP.insert(&key, &val, 0);
    }

    Ok(())
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}
