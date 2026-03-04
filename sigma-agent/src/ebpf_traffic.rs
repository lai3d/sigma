use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use aya::maps::HashMap as BpfHashMap;
use aya::programs::KProbe;
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

unsafe impl aya::Pod for TrafficKey {}
unsafe impl aya::Pod for TrafficValue {}
unsafe impl aya::Pod for RetransmitValue {}
unsafe impl aya::Pod for ConnValue {}

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
        Some(prog) => match KProbe::try_from(prog) {
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

    // Attach kretprobe to tcp_v4_connect (non-fatal)
    match ebpf.program_mut("tcp_v4_connect") {
        Some(prog) => match KProbe::try_from(prog) {
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
        Some(prog) => match KProbe::try_from(prog) {
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
        Some(prog) => match KProbe::try_from(prog) {
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
        Some(prog) => match KProbe::try_from(prog) {
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

    // Attach kretprobe to udp_recvmsg (non-fatal)
    match ebpf.program_mut("udp_recvmsg") {
        Some(prog) => match KProbe::try_from(prog) {
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

    // Aggregate by (process_name, container_id)
    // (bytes_sent, bytes_recv, udp_bytes_sent, udp_bytes_recv, retransmits, active_conns, total_conns)
    let mut aggregated: HashMap<(String, Option<String>), (u64, u64, u64, u64, u64, u64, u64)> = HashMap::new();

    for (key, value) in &raw_entries {
        let proc_name = resolve_process_name(key.pid, host_proc);
        let container_id = resolve_container_id(key.pid, host_proc);
        let entry = aggregated.entry((proc_name, container_id)).or_insert((0, 0, 0, 0, 0, 0, 0));
        entry.0 += value.bytes_sent;
        entry.1 += value.bytes_recv;
    }

    for (key, value) in &udp_entries {
        let proc_name = resolve_process_name(key.pid, host_proc);
        let container_id = resolve_container_id(key.pid, host_proc);
        let entry = aggregated.entry((proc_name, container_id)).or_insert((0, 0, 0, 0, 0, 0, 0));
        entry.2 += value.bytes_sent;
        entry.3 += value.bytes_recv;
    }

    for (key, value) in &retransmit_entries {
        let proc_name = resolve_process_name(key.pid, host_proc);
        let container_id = resolve_container_id(key.pid, host_proc);
        let entry = aggregated.entry((proc_name, container_id)).or_insert((0, 0, 0, 0, 0, 0, 0));
        entry.4 += value.count;
    }

    for (key, value) in &conn_entries {
        let proc_name = resolve_process_name(key.pid, host_proc);
        let container_id = resolve_container_id(key.pid, host_proc);
        let entry = aggregated.entry((proc_name, container_id)).or_insert((0, 0, 0, 0, 0, 0, 0));
        entry.5 += value.active;
        entry.6 += value.total;
    }

    let stats: Vec<ProcessTraffic> = aggregated
        .into_iter()
        .map(|((process_name, container_id), (bytes_sent, bytes_recv, udp_bytes_sent, udp_bytes_recv, retransmits, active_connections, total_connections))| ProcessTraffic {
            process_name,
            container_id,
            bytes_sent,
            bytes_recv,
            udp_bytes_sent,
            udp_bytes_recv,
            retransmits,
            active_connections,
            total_connections,
        })
        .collect();

    Ok(stats)
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
