#![no_std]
#![no_main]

use aya_ebpf::{
    helpers::{bpf_get_current_pid_tgid, bpf_ktime_get_ns, bpf_probe_read_kernel},
    macros::{kprobe, kretprobe, map, tracepoint},
    maps::HashMap,
    programs::{ProbeContext, RetProbeContext, TracePointContext},
};
use sigma_agent_ebpf_common::{ConnLatencyValue, ConnValue, DnsQueryValue, DropKey, DropValue, RetransmitValue, RttValue, TrafficKey, TrafficValue};

/// Offset of `skc_dport` within `struct sock` (Linux 5.x/6.x x86_64).
/// Located at sock.__sk_common.skc_dport. Network byte order (big-endian).
const SKC_DPORT_OFFSET: usize = 12;

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

/// Scratch map: stores entry timestamp (ns) for tcp_v4_connect kprobe, keyed by PID.
#[map]
static CONN_START_TS: HashMap<TrafficKey, u64> = HashMap::with_max_entries(8192, 0);

/// Per-PID TCP connection latency statistics (SYN-to-established time in microseconds).
#[map]
static CONN_LATENCY_MAP: HashMap<TrafficKey, ConnLatencyValue> = HashMap::with_max_entries(8192, 0);

/// Per-(PID, reason) packet drop counters from skb:kfree_skb tracepoint.
#[map]
static DROP_MAP: HashMap<DropKey, DropValue> = HashMap::with_max_entries(8192, 0);

/// Per-PID DNS query counters (UDP sends to port 53).
#[map]
static DNS_QUERY_MAP: HashMap<TrafficKey, DnsQueryValue> = HashMap::with_max_entries(8192, 0);

/// Offset of the `reason` field within the skb:kfree_skb tracepoint args.
/// Layout: trace_entry common header (8) + skbaddr(8) + location(8) + rx_sk(8) + protocol(2) + padding(2) = 36
const KFREE_SKB_REASON_OFFSET: usize = 36;

/// tracepoint on skb:kfree_skb — fires when a socket buffer is freed.
/// On Linux 5.17+, includes a `reason` field from `enum skb_drop_reason`.
#[tracepoint]
pub fn kfree_skb(ctx: TracePointContext) -> u32 {
    match try_kfree_skb(&ctx) {
        Ok(()) => 0,
        Err(_) => 0,
    }
}

fn try_kfree_skb(ctx: &TracePointContext) -> Result<(), i64> {
    // Read the drop reason at the known offset within the tracepoint args.
    // On kernels < 5.17, this read may fail — we return Ok to skip silently.
    let reason: u32 = unsafe {
        bpf_probe_read_kernel((ctx.as_ptr() as *const u8).add(KFREE_SKB_REASON_OFFSET) as *const u32)
            .map_err(|_| 1i64)?
    };

    // reason=0 means SKB_NOT_DROPPED_YET / normal free — not a real drop
    if reason == 0 {
        return Ok(());
    }

    let pid = (bpf_get_current_pid_tgid() >> 32) as u32;
    let key = DropKey { pid, reason };

    if let Some(val) = DROP_MAP.get_ptr_mut(&key) {
        unsafe {
            (*val).count += 1;
        }
    } else {
        let val = DropValue { count: 1 };
        let _ = DROP_MAP.insert(&key, &val, 0);
    }

    Ok(())
}

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

/// kprobe on tcp_v4_connect (entry) — captures start timestamp for connection latency.
#[kprobe]
pub fn tcp_v4_connect_entry(ctx: ProbeContext) -> u32 {
    match try_tcp_v4_connect_entry(&ctx) {
        Ok(()) => 0,
        Err(_) => 0,
    }
}

fn try_tcp_v4_connect_entry(_ctx: &ProbeContext) -> Result<(), i64> {
    let pid = (bpf_get_current_pid_tgid() >> 32) as u32;
    let key = TrafficKey { pid };
    let ts = unsafe { bpf_ktime_get_ns() };
    let _ = CONN_START_TS.insert(&key, &ts, 0);
    Ok(())
}

/// kretprobe on tcp_v4_connect — on success (ret==0), increments active and total connections
/// and computes connection latency from the entry timestamp.
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

    // Compute connection latency from entry timestamp
    if let Some(start_ts) = CONN_START_TS.get(&key) {
        let now = unsafe { bpf_ktime_get_ns() };
        let delta_us = (now - *start_ts) / 1000;
        let _ = CONN_START_TS.remove(&key);

        if delta_us > 0 {
            if let Some(val) = CONN_LATENCY_MAP.get_ptr_mut(&key) {
                unsafe {
                    (*val).count += 1;
                    (*val).sum_us += delta_us;
                    if delta_us < (*val).min_us {
                        (*val).min_us = delta_us;
                    }
                    if delta_us > (*val).max_us {
                        (*val).max_us = delta_us;
                    }
                }
            } else {
                let val = ConnLatencyValue {
                    count: 1,
                    sum_us: delta_us,
                    min_us: delta_us,
                    max_us: delta_us,
                };
                let _ = CONN_LATENCY_MAP.insert(&key, &val, 0);
            }
        }
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

/// kprobe on udp_sendmsg — DNS query tracing. Filters for destination port 53 only.
/// Reads `skc_dport` from the sock struct to detect DNS traffic (potential VPN DNS leaks).
#[kprobe]
pub fn dns_udp_sendmsg(ctx: ProbeContext) -> u32 {
    match try_dns_udp_sendmsg(&ctx) {
        Ok(()) => 0,
        Err(_) => 0,
    }
}

fn try_dns_udp_sendmsg(ctx: &ProbeContext) -> Result<(), i64> {
    let sk: *const u8 = ctx.arg(0).ok_or(1i64)?;
    if sk.is_null() {
        return Ok(());
    }

    // Read skc_dport from sock.__sk_common (network byte order)
    let raw_dport: u16 = unsafe {
        bpf_probe_read_kernel(sk.add(SKC_DPORT_OFFSET) as *const u16).map_err(|_| 1i64)?
    };
    let dport = u16::from_be(raw_dport);

    if dport != 53 {
        return Ok(());
    }

    let size: u64 = ctx.arg(2).ok_or(1i64)?;
    let pid = (bpf_get_current_pid_tgid() >> 32) as u32;
    let key = TrafficKey { pid };

    if let Some(val) = DNS_QUERY_MAP.get_ptr_mut(&key) {
        unsafe {
            (*val).queries += 1;
            (*val).bytes += size;
        }
    } else {
        let val = DnsQueryValue {
            queries: 1,
            bytes: size,
        };
        let _ = DNS_QUERY_MAP.insert(&key, &val, 0);
    }

    Ok(())
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}
