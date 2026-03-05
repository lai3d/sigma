use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use aya::maps::HashMap as BpfHashMap;
use aya::programs::{KProbe, TracePoint};
use aya::{Ebpf, EbpfLoader};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

// Local types with same repr(C) layout as sigma_agent_ebpf_common types.
// Needed because Rust orphan rules prevent implementing aya::Pod for external types.
#[repr(C)]
#[derive(Clone, Copy)]
struct TrafficKey {
    pid: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct TrafficValue {
    bytes_sent: u64,
    bytes_recv: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RetransmitValue {
    count: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ConnValue {
    active: u64,
    total: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RttValue {
    count: u64,
    sum_us: u64,
    min_us: u64,
    max_us: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ConnLatencyValue {
    count: u64,
    sum_us: u64,
    min_us: u64,
    max_us: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DropKey {
    pid: u32,
    reason: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DropValue {
    count: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DnsQueryValue {
    queries: u64,
    bytes: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ExecValue {
    count: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct OomKillValue {
    count: u64,
}

unsafe impl aya::Pod for TrafficKey {}
unsafe impl aya::Pod for TrafficValue {}
unsafe impl aya::Pod for RetransmitValue {}
unsafe impl aya::Pod for ConnValue {}
unsafe impl aya::Pod for RttValue {}
unsafe impl aya::Pod for ConnLatencyValue {}
unsafe impl aya::Pod for DropKey {}
unsafe impl aya::Pod for DropValue {}
unsafe impl aya::Pod for DnsQueryValue {}
unsafe impl aya::Pod for ExecValue {}
unsafe impl aya::Pod for OomKillValue {}

/// Per-process traffic stats resolved from eBPF data.
#[derive(Clone, Debug)]
pub struct ProcessTraffic {
    pub process_name: String,
    pub container_id: Option<String>,
    pub bytes_sent: u64,
    pub bytes_recv: u64,
    pub udp_bytes_sent: u64,
    pub udp_bytes_recv: u64,
    pub retransmits: u64,
    pub active_connections: u64,
    pub total_connections: u64,
    pub rtt_avg_us: u64,
    pub rtt_min_us: u64,
    pub rtt_max_us: u64,
    pub conn_latency_avg_us: u64,
    pub conn_latency_min_us: u64,
    pub conn_latency_max_us: u64,
    pub drops: Vec<(String, u64)>,
    pub dns_queries: u64,
    pub dns_bytes: u64,
    pub exec_count: u64,
    pub oom_kills: u64,
}

pub type SharedTrafficStats = Arc<RwLock<Vec<ProcessTraffic>>>;

/// Load the pre-compiled eBPF programs and attach kprobes.
pub fn load_ebpf() -> anyhow::Result<Ebpf> {
    // The eBPF bytecode is embedded at compile time from the build stage
    let ebpf_bytes = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/ebpf-programs/sigma-agent-ebpf"));

    let mut ebpf = EbpfLoader::new().load(ebpf_bytes)?;

    // Attach kprobe to tcp_sendmsg
    let sendmsg: &mut KProbe = ebpf.program_mut("tcp_sendmsg")
        .ok_or_else(|| anyhow::anyhow!("kprobe program 'tcp_sendmsg' not found"))?
        .try_into()?;
    sendmsg.load()?;
    sendmsg.attach("tcp_sendmsg", 0)?;
    info!("Attached kprobe to tcp_sendmsg");

    // Attach kretprobe to tcp_recvmsg
    let recvmsg: &mut KProbe = ebpf.program_mut("tcp_recvmsg")
        .ok_or_else(|| anyhow::anyhow!("kretprobe program 'tcp_recvmsg' not found"))?
        .try_into()?;
    recvmsg.load()?;
    recvmsg.attach("tcp_recvmsg", 0)?;
    info!("Attached kretprobe to tcp_recvmsg");

    // Attach kprobe to tcp_retransmit_skb (non-fatal)
    match ebpf.program_mut("tcp_retransmit_skb") {
        Some(prog) => match <&mut KProbe>::try_from(prog) {
            Ok(kp) => {
                if let Err(e) = kp.load().and_then(|()| kp.attach("tcp_retransmit_skb", 0)) {
                    warn!("Failed to attach kprobe to tcp_retransmit_skb: {:#}", e);
                } else {
                    info!("Attached kprobe to tcp_retransmit_skb");
                }
            }
            Err(e) => warn!("tcp_retransmit_skb program type mismatch: {:#}", e),
        },
        None => warn!("kprobe program 'tcp_retransmit_skb' not found in eBPF object"),
    }

    // Attach kprobe to tcp_v4_connect entry (non-fatal — connection latency)
    match ebpf.program_mut("tcp_v4_connect_entry") {
        Some(prog) => match <&mut KProbe>::try_from(prog) {
            Ok(kp) => {
                if let Err(e) = kp.load().and_then(|()| kp.attach("tcp_v4_connect", 0)) {
                    warn!("Failed to attach kprobe to tcp_v4_connect (entry): {:#}", e);
                } else {
                    info!("Attached kprobe to tcp_v4_connect (entry)");
                }
            }
            Err(e) => warn!("tcp_v4_connect_entry program type mismatch: {:#}", e),
        },
        None => warn!("kprobe program 'tcp_v4_connect_entry' not found in eBPF object"),
    }

    // Attach kretprobe to tcp_v4_connect (non-fatal)
    match ebpf.program_mut("tcp_v4_connect") {
        Some(prog) => match <&mut KProbe>::try_from(prog) {
            Ok(kp) => {
                if let Err(e) = kp.load().and_then(|()| kp.attach("tcp_v4_connect", 0)) {
                    warn!("Failed to attach kretprobe to tcp_v4_connect: {:#}", e);
                } else {
                    info!("Attached kretprobe to tcp_v4_connect");
                }
            }
            Err(e) => warn!("tcp_v4_connect program type mismatch: {:#}", e),
        },
        None => warn!("kretprobe program 'tcp_v4_connect' not found in eBPF object"),
    }

    // Attach kprobe to tcp_close (non-fatal)
    match ebpf.program_mut("tcp_close") {
        Some(prog) => match <&mut KProbe>::try_from(prog) {
            Ok(kp) => {
                if let Err(e) = kp.load().and_then(|()| kp.attach("tcp_close", 0)) {
                    warn!("Failed to attach kprobe to tcp_close: {:#}", e);
                } else {
                    info!("Attached kprobe to tcp_close");
                }
            }
            Err(e) => warn!("tcp_close program type mismatch: {:#}", e),
        },
        None => warn!("kprobe program 'tcp_close' not found in eBPF object"),
    }

    // Attach kretprobe to inet_csk_accept (non-fatal)
    match ebpf.program_mut("inet_csk_accept") {
        Some(prog) => match <&mut KProbe>::try_from(prog) {
            Ok(kp) => {
                if let Err(e) = kp.load().and_then(|()| kp.attach("inet_csk_accept", 0)) {
                    warn!("Failed to attach kretprobe to inet_csk_accept: {:#}", e);
                } else {
                    info!("Attached kretprobe to inet_csk_accept");
                }
            }
            Err(e) => warn!("inet_csk_accept program type mismatch: {:#}", e),
        },
        None => warn!("kretprobe program 'inet_csk_accept' not found in eBPF object"),
    }

    // Attach kprobe to udp_sendmsg (non-fatal)
    match ebpf.program_mut("udp_sendmsg") {
        Some(prog) => match <&mut KProbe>::try_from(prog) {
            Ok(kp) => {
                if let Err(e) = kp.load().and_then(|()| kp.attach("udp_sendmsg", 0)) {
                    warn!("Failed to attach kprobe to udp_sendmsg: {:#}", e);
                } else {
                    info!("Attached kprobe to udp_sendmsg");
                }
            }
            Err(e) => warn!("udp_sendmsg program type mismatch: {:#}", e),
        },
        None => warn!("kprobe program 'udp_sendmsg' not found in eBPF object"),
    }

    // Attach kprobe to tcp_rcv_established (non-fatal — RTT tracking)
    match ebpf.program_mut("tcp_rcv_established") {
        Some(prog) => match <&mut KProbe>::try_from(prog) {
            Ok(kp) => {
                if let Err(e) = kp.load().and_then(|()| kp.attach("tcp_rcv_established", 0)) {
                    warn!("Failed to attach kprobe to tcp_rcv_established: {:#}", e);
                } else {
                    info!("Attached kprobe to tcp_rcv_established");
                }
            }
            Err(e) => warn!("tcp_rcv_established program type mismatch: {:#}", e),
        },
        None => warn!("kprobe program 'tcp_rcv_established' not found in eBPF object"),
    }

    // Attach kretprobe to udp_recvmsg (non-fatal)
    match ebpf.program_mut("udp_recvmsg") {
        Some(prog) => match <&mut KProbe>::try_from(prog) {
            Ok(kp) => {
                if let Err(e) = kp.load().and_then(|()| kp.attach("udp_recvmsg", 0)) {
                    warn!("Failed to attach kretprobe to udp_recvmsg: {:#}", e);
                } else {
                    info!("Attached kretprobe to udp_recvmsg");
                }
            }
            Err(e) => warn!("udp_recvmsg program type mismatch: {:#}", e),
        },
        None => warn!("kretprobe program 'udp_recvmsg' not found in eBPF object"),
    }

    // Attach tracepoint to skb/kfree_skb (non-fatal — requires Linux 5.17+ for reason field)
    match ebpf.program_mut("kfree_skb") {
        Some(prog) => match <&mut TracePoint>::try_from(prog) {
            Ok(tp) => {
                if let Err(e) = tp.load().and_then(|()| tp.attach("skb", "kfree_skb")) {
                    warn!("Failed to attach tracepoint to skb/kfree_skb: {:#}", e);
                } else {
                    info!("Attached tracepoint to skb/kfree_skb");
                }
            }
            Err(e) => warn!("kfree_skb program type mismatch: {:#}", e),
        },
        None => warn!("tracepoint program 'kfree_skb' not found in eBPF object"),
    }

    // Attach kprobe to udp_sendmsg for DNS query tracing (non-fatal)
    match ebpf.program_mut("dns_udp_sendmsg") {
        Some(prog) => match <&mut KProbe>::try_from(prog) {
            Ok(kp) => {
                if let Err(e) = kp.load().and_then(|()| kp.attach("udp_sendmsg", 0)) {
                    warn!("Failed to attach kprobe to udp_sendmsg (DNS): {:#}", e);
                } else {
                    info!("Attached kprobe to udp_sendmsg (DNS)");
                }
            }
            Err(e) => warn!("dns_udp_sendmsg program type mismatch: {:#}", e),
        },
        None => warn!("kprobe program 'dns_udp_sendmsg' not found in eBPF object"),
    }

    // Attach tracepoint to sched/sched_process_exec (non-fatal — exec tracing for intrusion detection)
    match ebpf.program_mut("sched_process_exec") {
        Some(prog) => match <&mut TracePoint>::try_from(prog) {
            Ok(tp) => {
                if let Err(e) = tp.load().and_then(|()| tp.attach("sched", "sched_process_exec")) {
                    warn!("Failed to attach tracepoint to sched/sched_process_exec: {:#}", e);
                } else {
                    info!("Attached tracepoint to sched/sched_process_exec");
                }
            }
            Err(e) => warn!("sched_process_exec program type mismatch: {:#}", e),
        },
        None => warn!("tracepoint program 'sched_process_exec' not found in eBPF object"),
    }

    // Attach tracepoint to oom/mark_victim (non-fatal — OOM kill tracking)
    match ebpf.program_mut("oom_mark_victim") {
        Some(prog) => match <&mut TracePoint>::try_from(prog) {
            Ok(tp) => {
                if let Err(e) = tp.load().and_then(|()| tp.attach("oom", "mark_victim")) {
                    warn!("Failed to attach tracepoint to oom/mark_victim: {:#}", e);
                } else {
                    info!("Attached tracepoint to oom/mark_victim");
                }
            }
            Err(e) => warn!("oom_mark_victim program type mismatch: {:#}", e),
        },
        None => warn!("tracepoint program 'oom_mark_victim' not found in eBPF object"),
    }

    Ok(ebpf)
}

/// Main loop: periodically reads the BPF map, resolves PIDs, and updates shared stats.
pub async fn traffic_loop(
    mut ebpf: Ebpf,
    shared_stats: SharedTrafficStats,
    interval_secs: u64,
    host_proc: String,
) {
    let interval = Duration::from_secs(interval_secs);

    loop {
        tokio::time::sleep(interval).await;

        match harvest_traffic(&mut ebpf, &host_proc) {
            Ok(stats) => {
                debug!(entries = stats.len(), "Harvested eBPF traffic stats");
                let mut guard = shared_stats.write().await;
                *guard = stats;
            }
            Err(e) => {
                warn!("Failed to harvest eBPF traffic stats: {:#}", e);
            }
        }
    }
}

/// Read all entries from the BPF map, resolve PIDs, aggregate by process name.
fn harvest_traffic(ebpf: &mut Ebpf, host_proc: &str) -> anyhow::Result<Vec<ProcessTraffic>> {
    let map = ebpf.map_mut("TRAFFIC_MAP")
        .ok_or_else(|| anyhow::anyhow!("BPF map 'TRAFFIC_MAP' not found"))?;
    let mut traffic_map = BpfHashMap::<&mut aya::maps::MapData, TrafficKey, TrafficValue>::try_from(map)?;

    // Collect all entries first, then clear
    let mut raw_entries: Vec<(TrafficKey, TrafficValue)> = Vec::new();
    let mut keys_to_delete: Vec<TrafficKey> = Vec::new();

    // Iterate and collect
    for item in traffic_map.iter() {
        match item {
            Ok((key, value)) => {
                raw_entries.push((key, value));
                keys_to_delete.push(key);
            }
            Err(e) => {
                debug!("Error reading BPF map entry: {}", e);
            }
        }
    }

    // Clear collected entries
    for key in &keys_to_delete {
        let _ = traffic_map.remove(key);
    }

    // --- Read UDP_TRAFFIC_MAP (read-and-clear, like TRAFFIC_MAP) ---
    let mut udp_entries: Vec<(TrafficKey, TrafficValue)> = Vec::new();
    if let Some(map) = ebpf.map_mut("UDP_TRAFFIC_MAP") {
        if let Ok(mut udp_map) = BpfHashMap::<&mut aya::maps::MapData, TrafficKey, TrafficValue>::try_from(map) {
            let mut udp_keys: Vec<TrafficKey> = Vec::new();
            for item in udp_map.iter() {
                match item {
                    Ok((key, value)) => {
                        udp_entries.push((key, value));
                        udp_keys.push(key);
                    }
                    Err(e) => debug!("Error reading UDP_TRAFFIC_MAP entry: {}", e),
                }
            }
            for key in &udp_keys {
                let _ = udp_map.remove(key);
            }
        }
    }

    // --- Read RETRANSMIT_MAP (read-and-clear, like TRAFFIC_MAP) ---
    let mut retransmit_entries: Vec<(TrafficKey, RetransmitValue)> = Vec::new();
    if let Some(map) = ebpf.map_mut("RETRANSMIT_MAP") {
        if let Ok(mut retransmit_map) = BpfHashMap::<&mut aya::maps::MapData, TrafficKey, RetransmitValue>::try_from(map) {
            let mut retransmit_keys: Vec<TrafficKey> = Vec::new();
            for item in retransmit_map.iter() {
                match item {
                    Ok((key, value)) => {
                        retransmit_entries.push((key, value));
                        retransmit_keys.push(key);
                    }
                    Err(e) => debug!("Error reading RETRANSMIT_MAP entry: {}", e),
                }
            }
            for key in &retransmit_keys {
                let _ = retransmit_map.remove(key);
            }
        }
    }

    // --- Read RTT_MAP (read-and-clear, like TRAFFIC_MAP) ---
    let mut rtt_entries: Vec<(TrafficKey, RttValue)> = Vec::new();
    if let Some(map) = ebpf.map_mut("RTT_MAP") {
        if let Ok(mut rtt_map) = BpfHashMap::<&mut aya::maps::MapData, TrafficKey, RttValue>::try_from(map) {
            let mut rtt_keys: Vec<TrafficKey> = Vec::new();
            for item in rtt_map.iter() {
                match item {
                    Ok((key, value)) => {
                        rtt_entries.push((key, value));
                        rtt_keys.push(key);
                    }
                    Err(e) => debug!("Error reading RTT_MAP entry: {}", e),
                }
            }
            for key in &rtt_keys {
                let _ = rtt_map.remove(key);
            }
        }
    }

    // --- Read CONN_LATENCY_MAP (read-and-clear, like RTT_MAP) ---
    let mut conn_latency_entries: Vec<(TrafficKey, ConnLatencyValue)> = Vec::new();
    if let Some(map) = ebpf.map_mut("CONN_LATENCY_MAP") {
        if let Ok(mut cl_map) = BpfHashMap::<&mut aya::maps::MapData, TrafficKey, ConnLatencyValue>::try_from(map) {
            let mut cl_keys: Vec<TrafficKey> = Vec::new();
            for item in cl_map.iter() {
                match item {
                    Ok((key, value)) => {
                        conn_latency_entries.push((key, value));
                        cl_keys.push(key);
                    }
                    Err(e) => debug!("Error reading CONN_LATENCY_MAP entry: {}", e),
                }
            }
            for key in &cl_keys {
                let _ = cl_map.remove(key);
            }
        }
    }

    // --- Read CONN_MAP (don't clear — active is a gauge) ---
    let mut conn_entries: Vec<(TrafficKey, ConnValue)> = Vec::new();
    if let Some(map) = ebpf.map_mut("CONN_MAP") {
        if let Ok(mut conn_map) = BpfHashMap::<&mut aya::maps::MapData, TrafficKey, ConnValue>::try_from(map) {
            let mut zero_keys: Vec<TrafficKey> = Vec::new();
            for item in conn_map.iter() {
                match item {
                    Ok((key, value)) => {
                        if value.active == 0 && value.total == 0 {
                            zero_keys.push(key);
                        } else {
                            conn_entries.push((key, value));
                        }
                    }
                    Err(e) => debug!("Error reading CONN_MAP entry: {}", e),
                }
            }
            // Clean up entries with zero active+total to prevent map from growing
            for key in &zero_keys {
                let _ = conn_map.remove(key);
            }
        }
    }

    // --- Read DROP_MAP (read-and-clear, like TRAFFIC_MAP) ---
    let mut drop_entries: Vec<(DropKey, DropValue)> = Vec::new();
    if let Some(map) = ebpf.map_mut("DROP_MAP") {
        if let Ok(mut drop_map) = BpfHashMap::<&mut aya::maps::MapData, DropKey, DropValue>::try_from(map) {
            let mut drop_keys: Vec<DropKey> = Vec::new();
            for item in drop_map.iter() {
                match item {
                    Ok((key, value)) => {
                        drop_entries.push((key, value));
                        drop_keys.push(key);
                    }
                    Err(e) => debug!("Error reading DROP_MAP entry: {}", e),
                }
            }
            for key in &drop_keys {
                let _ = drop_map.remove(key);
            }
        }
    }

    // --- Read DNS_QUERY_MAP (read-and-clear, like TRAFFIC_MAP) ---
    let mut dns_entries: Vec<(TrafficKey, DnsQueryValue)> = Vec::new();
    if let Some(map) = ebpf.map_mut("DNS_QUERY_MAP") {
        if let Ok(mut dns_map) = BpfHashMap::<&mut aya::maps::MapData, TrafficKey, DnsQueryValue>::try_from(map) {
            let mut dns_keys: Vec<TrafficKey> = Vec::new();
            for item in dns_map.iter() {
                match item {
                    Ok((key, value)) => {
                        dns_entries.push((key, value));
                        dns_keys.push(key);
                    }
                    Err(e) => debug!("Error reading DNS_QUERY_MAP entry: {}", e),
                }
            }
            for key in &dns_keys {
                let _ = dns_map.remove(key);
            }
        }
    }

    // --- Read EXEC_MAP (read-and-clear, like TRAFFIC_MAP) ---
    let mut exec_entries: Vec<(TrafficKey, ExecValue)> = Vec::new();
    if let Some(map) = ebpf.map_mut("EXEC_MAP") {
        if let Ok(mut exec_map) = BpfHashMap::<&mut aya::maps::MapData, TrafficKey, ExecValue>::try_from(map) {
            let mut exec_keys: Vec<TrafficKey> = Vec::new();
            for item in exec_map.iter() {
                match item {
                    Ok((key, value)) => {
                        exec_entries.push((key, value));
                        exec_keys.push(key);
                    }
                    Err(e) => debug!("Error reading EXEC_MAP entry: {}", e),
                }
            }
            for key in &exec_keys {
                let _ = exec_map.remove(key);
            }
        }
    }

    // --- Read OOM_KILL_MAP (read-and-clear, like EXEC_MAP) ---
    let mut oom_entries: Vec<(TrafficKey, OomKillValue)> = Vec::new();
    if let Some(map) = ebpf.map_mut("OOM_KILL_MAP") {
        if let Ok(mut oom_map) = BpfHashMap::<&mut aya::maps::MapData, TrafficKey, OomKillValue>::try_from(map) {
            let mut oom_keys: Vec<TrafficKey> = Vec::new();
            for item in oom_map.iter() {
                match item {
                    Ok((key, value)) => {
                        oom_entries.push((key, value));
                        oom_keys.push(key);
                    }
                    Err(e) => debug!("Error reading OOM_KILL_MAP entry: {}", e),
                }
            }
            for key in &oom_keys {
                let _ = oom_map.remove(key);
            }
        }
    }

    // Aggregate by (process_name, container_id)
    // Fields: bytes_sent, bytes_recv, udp_bytes_sent, udp_bytes_recv, retransmits,
    //         active_conns, total_conns, rtt_count, rtt_sum_us, rtt_min_us, rtt_max_us, drops
    #[derive(Default)]
    struct Agg {
        bytes_sent: u64,
        bytes_recv: u64,
        udp_bytes_sent: u64,
        udp_bytes_recv: u64,
        retransmits: u64,
        active_conns: u64,
        total_conns: u64,
        rtt_count: u64,
        rtt_sum_us: u64,
        rtt_min_us: u64,
        rtt_max_us: u64,
        conn_latency_count: u64,
        conn_latency_sum_us: u64,
        conn_latency_min_us: u64,
        conn_latency_max_us: u64,
        drops: HashMap<u32, u64>,
        dns_queries: u64,
        dns_bytes: u64,
        exec_count: u64,
        oom_kills: u64,
    }

    let mut aggregated: HashMap<(String, Option<String>), Agg> = HashMap::new();

    for (key, value) in &raw_entries {
        let proc_name = resolve_process_name(key.pid, host_proc);
        let container_id = resolve_container_id(key.pid, host_proc);
        let entry = aggregated.entry((proc_name, container_id)).or_default();
        entry.bytes_sent += value.bytes_sent;
        entry.bytes_recv += value.bytes_recv;
    }

    for (key, value) in &udp_entries {
        let proc_name = resolve_process_name(key.pid, host_proc);
        let container_id = resolve_container_id(key.pid, host_proc);
        let entry = aggregated.entry((proc_name, container_id)).or_default();
        entry.udp_bytes_sent += value.bytes_sent;
        entry.udp_bytes_recv += value.bytes_recv;
    }

    for (key, value) in &retransmit_entries {
        let proc_name = resolve_process_name(key.pid, host_proc);
        let container_id = resolve_container_id(key.pid, host_proc);
        let entry = aggregated.entry((proc_name, container_id)).or_default();
        entry.retransmits += value.count;
    }

    for (key, value) in &rtt_entries {
        let proc_name = resolve_process_name(key.pid, host_proc);
        let container_id = resolve_container_id(key.pid, host_proc);
        let entry = aggregated.entry((proc_name, container_id)).or_default();
        entry.rtt_count += value.count;
        entry.rtt_sum_us += value.sum_us;
        if entry.rtt_min_us == 0 || value.min_us < entry.rtt_min_us {
            entry.rtt_min_us = value.min_us;
        }
        if value.max_us > entry.rtt_max_us {
            entry.rtt_max_us = value.max_us;
        }
    }

    for (key, value) in &conn_entries {
        let proc_name = resolve_process_name(key.pid, host_proc);
        let container_id = resolve_container_id(key.pid, host_proc);
        let entry = aggregated.entry((proc_name, container_id)).or_default();
        entry.active_conns += value.active;
        entry.total_conns += value.total;
    }

    for (key, value) in &conn_latency_entries {
        let proc_name = resolve_process_name(key.pid, host_proc);
        let container_id = resolve_container_id(key.pid, host_proc);
        let entry = aggregated.entry((proc_name, container_id)).or_default();
        entry.conn_latency_count += value.count;
        entry.conn_latency_sum_us += value.sum_us;
        if entry.conn_latency_min_us == 0 || value.min_us < entry.conn_latency_min_us {
            entry.conn_latency_min_us = value.min_us;
        }
        if value.max_us > entry.conn_latency_max_us {
            entry.conn_latency_max_us = value.max_us;
        }
    }

    for (key, value) in &drop_entries {
        let proc_name = resolve_process_name(key.pid, host_proc);
        let container_id = resolve_container_id(key.pid, host_proc);
        let entry = aggregated.entry((proc_name, container_id)).or_default();
        *entry.drops.entry(key.reason).or_insert(0) += value.count;
    }

    for (key, value) in &dns_entries {
        let proc_name = resolve_process_name(key.pid, host_proc);
        let container_id = resolve_container_id(key.pid, host_proc);
        let entry = aggregated.entry((proc_name, container_id)).or_default();
        entry.dns_queries += value.queries;
        entry.dns_bytes += value.bytes;
    }

    for (key, value) in &exec_entries {
        let proc_name = resolve_process_name(key.pid, host_proc);
        let container_id = resolve_container_id(key.pid, host_proc);
        let entry = aggregated.entry((proc_name, container_id)).or_default();
        entry.exec_count += value.count;
    }

    for (key, value) in &oom_entries {
        let proc_name = resolve_process_name(key.pid, host_proc);
        let container_id = resolve_container_id(key.pid, host_proc);
        let entry = aggregated.entry((proc_name, container_id)).or_default();
        entry.oom_kills += value.count;
    }

    let stats: Vec<ProcessTraffic> = aggregated
        .into_iter()
        .map(|((process_name, container_id), agg)| {
            let rtt_avg_us = if agg.rtt_count > 0 { agg.rtt_sum_us / agg.rtt_count } else { 0 };
            let conn_latency_avg_us = if agg.conn_latency_count > 0 { agg.conn_latency_sum_us / agg.conn_latency_count } else { 0 };
            let drops: Vec<(String, u64)> = agg.drops
                .into_iter()
                .map(|(reason, count)| (drop_reason_name(reason), count))
                .collect();
            ProcessTraffic {
                process_name,
                container_id,
                bytes_sent: agg.bytes_sent,
                bytes_recv: agg.bytes_recv,
                udp_bytes_sent: agg.udp_bytes_sent,
                udp_bytes_recv: agg.udp_bytes_recv,
                retransmits: agg.retransmits,
                active_connections: agg.active_conns,
                total_connections: agg.total_conns,
                rtt_avg_us,
                rtt_min_us: agg.rtt_min_us,
                rtt_max_us: agg.rtt_max_us,
                conn_latency_avg_us,
                conn_latency_min_us: agg.conn_latency_min_us,
                conn_latency_max_us: agg.conn_latency_max_us,
                drops,
                dns_queries: agg.dns_queries,
                dns_bytes: agg.dns_bytes,
                exec_count: agg.exec_count,
                oom_kills: agg.oom_kills,
            }
        })
        .collect();

    Ok(stats)
}

/// Map kernel `enum skb_drop_reason` values to human-readable names.
/// Values from include/net/dropreason-core.h (Linux 6.x).
fn drop_reason_name(reason: u32) -> String {
    match reason {
        1 => "NOT_SPECIFIED".into(),
        2 => "NO_SOCKET".into(),
        3 => "PKT_TOO_SMALL".into(),
        4 => "TCP_CSUM".into(),
        5 => "SOCKET_FILTER".into(),
        6 => "UDP_CSUM".into(),
        7 => "NETFILTER_DROP".into(),
        8 => "OTHERHOST".into(),
        9 => "IP_CSUM".into(),
        10 => "IP_INHDR".into(),
        11 => "IP_RPFILTER".into(),
        12 => "UNICAST_IN_L2_MULTICAST".into(),
        13 => "XFRM_POLICY".into(),
        14 => "IP_NOPROTO".into(),
        15 => "SOCKET_RCVBUFF".into(),
        16 => "PROTO_MEM".into(),
        17 => "TCP_MD5NOTFOUND".into(),
        18 => "TCP_MD5UNEXPECTED".into(),
        19 => "TCP_MD5FAILURE".into(),
        20 => "SOCKET_BACKLOG".into(),
        21 => "TCP_FLAGS".into(),
        22 => "TCP_ZEROWINDOW".into(),
        23 => "TCP_OLD_DATA".into(),
        24 => "TCP_OVERWINDOW".into(),
        25 => "TCP_OFOMERGE".into(),
        26 => "TCP_RFC7323_PAWS".into(),
        27 => "TCP_INVALID_SEQUENCE".into(),
        28 => "TCP_RESET".into(),
        29 => "TCP_INVALID_SYN".into(),
        30 => "TCP_CLOSE".into(),
        31 => "TCP_FASTOPEN".into(),
        32 => "TCP_OLD_ACK".into(),
        33 => "TCP_TOO_OLD_ACK".into(),
        34 => "TCP_ACK_UNSENT_DATA".into(),
        35 => "TCP_OFO_QUEUE_PRUNE".into(),
        36 => "TCP_OFO_DROP".into(),
        37 => "IP_OUTNOROUTES".into(),
        38 => "BPF_CGROUP_EGRESS".into(),
        39 => "IPV6DISABLED".into(),
        40 => "NEIGH_CREATEFAIL".into(),
        41 => "NEIGH_FAILED".into(),
        42 => "NEIGH_QUEUEFULL".into(),
        43 => "NEIGH_DEAD".into(),
        44 => "TC_EGRESS".into(),
        45 => "QDISC_DROP".into(),
        46 => "CPU_BACKLOG".into(),
        47 => "XDP".into(),
        48 => "TC_INGRESS".into(),
        49 => "UNHANDLED_PROTO".into(),
        50 => "SKB_CSUM".into(),
        51 => "SKB_GSO_SEG".into(),
        52 => "SKB_UCOPY_FAULT".into(),
        53 => "DEV_HDR".into(),
        54 => "DEV_READY".into(),
        55 => "FULL_RING".into(),
        56 => "NOMEM".into(),
        57 => "HDR_TRUNC".into(),
        58 => "TAP_FILTER".into(),
        59 => "TAP_TXFILTER".into(),
        60 => "ICMP_CSUM".into(),
        61 => "INVALID_PROTO".into(),
        62 => "IP_INADDRERRORS".into(),
        63 => "IP_INNOROUTES".into(),
        64 => "PKT_TOO_BIG".into(),
        65 => "DUP_FRAG".into(),
        66 => "FRAG_REASM_TIMEOUT".into(),
        67 => "FRAG_TOO_FAR".into(),
        _ => format!("UNKNOWN_{}", reason),
    }
}

/// Read /proc/<pid>/comm to get the process name.
fn resolve_process_name(pid: u32, host_proc: &str) -> String {
    let comm_path = format!("{}/{}/comm", host_proc, pid);
    match std::fs::read_to_string(&comm_path) {
        Ok(name) => name.trim().to_string(),
        Err(_) => format!("pid-{}", pid),
    }
}

/// Try to extract Docker/containerd container ID from /proc/<pid>/cgroup.
fn resolve_container_id(pid: u32, host_proc: &str) -> Option<String> {
    let cgroup_path = format!("{}/{}/cgroup", host_proc, pid);
    let content = std::fs::read_to_string(&cgroup_path).ok()?;

    for line in content.lines() {
        // Docker: .../docker-<64-hex-chars>.scope
        if let Some(pos) = line.find("docker-") {
            let rest = &line[pos + 7..];
            if let Some(dot) = rest.find('.') {
                let id = &rest[..dot];
                if id.len() >= 12 && id.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Some(id[..12].to_string());
                }
            }
        }
        // containerd: .../cri-containerd-<64-hex-chars>.scope
        if let Some(pos) = line.find("cri-containerd-") {
            let rest = &line[pos + 15..];
            if let Some(dot) = rest.find('.') {
                let id = &rest[..dot];
                if id.len() >= 12 && id.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Some(id[..12].to_string());
                }
            }
        }
        // Docker cgroup v1: .../docker/<64-hex-chars>
        if let Some(pos) = line.find("/docker/") {
            let id = &line[pos + 8..];
            if id.len() >= 12 && id.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some(id[..12].to_string());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_container_id_docker_scope() {
        // Simulates Docker cgroup v2 format
        let tmpdir = tempfile::tempdir().unwrap();
        let pid_dir = tmpdir.path().join("12345");
        std::fs::create_dir_all(&pid_dir).unwrap();
        std::fs::write(
            pid_dir.join("cgroup"),
            "0::/system.slice/docker-abc123def456789012345678901234567890123456789012345678901234.scope\n",
        ).unwrap();

        let result = resolve_container_id(12345, tmpdir.path().to_str().unwrap());
        assert_eq!(result, Some("abc123def456".to_string()));
    }

    #[test]
    fn test_resolve_container_id_none() {
        let tmpdir = tempfile::tempdir().unwrap();
        let pid_dir = tmpdir.path().join("12345");
        std::fs::create_dir_all(&pid_dir).unwrap();
        std::fs::write(pid_dir.join("cgroup"), "0::/init.scope\n").unwrap();

        let result = resolve_container_id(12345, tmpdir.path().to_str().unwrap());
        assert_eq!(result, None);
    }

    #[test]
    fn test_resolve_process_name() {
        let tmpdir = tempfile::tempdir().unwrap();
        let pid_dir = tmpdir.path().join("42");
        std::fs::create_dir_all(&pid_dir).unwrap();
        std::fs::write(pid_dir.join("comm"), "envoy\n").unwrap();

        let result = resolve_process_name(42, tmpdir.path().to_str().unwrap());
        assert_eq!(result, "envoy");
    }

    #[test]
    fn test_resolve_process_name_missing_pid() {
        let result = resolve_process_name(99999999, "/proc");
        assert_eq!(result, "pid-99999999");
    }
}
