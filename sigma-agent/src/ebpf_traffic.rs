use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use aya::maps::HashMap as BpfHashMap;
use aya::programs::{KProbe, TracePoint};
use aya::{Ebpf, EbpfLoader};
use sigma_agent_ebpf_common::{TrafficKey, TrafficValue};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

// Implement Pod for our shared types so aya can read them from BPF maps
unsafe impl aya::Pod for TrafficKey {}
unsafe impl aya::Pod for TrafficValue {}

/// Per-process traffic stats resolved from eBPF data.
#[derive(Clone, Debug)]
pub struct ProcessTraffic {
    pub process_name: String,
    pub container_id: Option<String>,
    pub bytes_sent: u64,
    pub bytes_recv: u64,
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

    // Aggregate by (process_name, container_id)
    let mut aggregated: HashMap<(String, Option<String>), (u64, u64)> = HashMap::new();

    for (key, value) in &raw_entries {
        let proc_name = resolve_process_name(key.pid, host_proc);
        let container_id = resolve_container_id(key.pid, host_proc);
        let agg_key = (proc_name, container_id);

        let entry = aggregated.entry(agg_key).or_insert((0, 0));
        entry.0 += value.bytes_sent;
        entry.1 += value.bytes_recv;
    }

    let stats: Vec<ProcessTraffic> = aggregated
        .into_iter()
        .map(|((process_name, container_id), (bytes_sent, bytes_recv))| ProcessTraffic {
            process_name,
            container_id,
            bytes_sent,
            bytes_recv,
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
