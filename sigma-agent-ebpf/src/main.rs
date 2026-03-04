#![no_std]
#![no_main]

use aya_ebpf::{
    helpers::bpf_get_current_pid_tgid,
    macros::{kprobe, kretprobe, map},
    maps::HashMap,
    programs::{ProbeContext, RetProbeContext},
};
use sigma_agent_ebpf_common::{TrafficKey, TrafficValue};

/// Per-PID traffic counters. Userspace reads and periodically clears this map.
#[map]
static TRAFFIC_MAP: HashMap<TrafficKey, TrafficValue> = HashMap::with_max_entries(8192, 0);

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

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}
