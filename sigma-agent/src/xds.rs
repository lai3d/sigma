use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::sync::{mpsc, Notify, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use tracing::{info, warn};

use xds_api::pb::envoy::service::discovery::v3::{
    aggregated_discovery_service_server::{
        AggregatedDiscoveryService, AggregatedDiscoveryServiceServer,
    },
    DeltaDiscoveryRequest, DeltaDiscoveryResponse, DiscoveryRequest, DiscoveryResponse,
};
use xds_api::pb::google::protobuf::Any as XdsAny;

use crate::client::SigmaClient;
use crate::models::{EnvoyNode, EnvoyRoute, PaginatedResponse};
use crate::xds_resources;

/// Per-node xDS configuration snapshot.
struct NodeConfig {
    version: String,
    listeners: Vec<XdsAny>,
    clusters: Vec<XdsAny>,
}

/// All node configs keyed by node_id.
struct XdsState {
    configs: HashMap<String, NodeConfig>,
}

/// gRPC ADS server that polls sigma-api for config and pushes to connected Envoy clients.
///
/// Serves ALL active envoy_nodes for this VPS. Each connecting Envoy client is matched
/// by its `node.id` to the corresponding envoy_node config.
#[derive(Clone)]
pub struct XdsServer {
    client: Arc<SigmaClient>,
    vps_id: uuid::Uuid,
    poll_interval: u64,
    state: Arc<RwLock<XdsState>>,
    notify: Arc<Notify>,
}

impl XdsServer {
    pub fn new(
        client: Arc<SigmaClient>,
        vps_id: uuid::Uuid,
        poll_interval: u64,
    ) -> Self {
        Self {
            client,
            vps_id,
            poll_interval,
            state: Arc::new(RwLock::new(XdsState {
                configs: HashMap::new(),
            })),
            notify: Arc::new(Notify::new()),
        }
    }

    /// Run the config polling loop. Fetches immediately, then every poll_interval seconds.
    pub async fn config_poll_loop(self) {
        // Fetch immediately on startup
        match self.fetch_and_update_configs().await {
            Ok(n) if n > 0 => info!(nodes = n, "xDS initial config loaded"),
            Ok(_) => info!("xDS no envoy nodes found for this VPS yet"),
            Err(e) => warn!("xDS initial config fetch failed: {:#}", e),
        }

        loop {
            tokio::time::sleep(Duration::from_secs(self.poll_interval)).await;
            match self.fetch_and_update_configs().await {
                Ok(_) => {}
                Err(e) => warn!("xDS config poll failed: {:#}", e),
            }
        }
    }

    /// Fetch all envoy nodes for this VPS and their routes, update configs if changed.
    /// Returns the number of nodes with updated configs.
    async fn fetch_and_update_configs(&self) -> Result<usize> {
        // Fetch all active envoy nodes for this VPS
        let nodes: PaginatedResponse<EnvoyNode> = self
            .client
            .get(&format!(
                "/envoy-nodes?vps_id={}&status=active&per_page=100",
                self.vps_id
            ))
            .await?;

        let mut updated = 0;

        for node in &nodes.data {
            // Check if version changed
            let needs_update = {
                let state = self.state.read().await;
                match state.configs.get(&node.node_id) {
                    Some(cfg) => cfg.version != node.config_version.to_string(),
                    None => true, // new node
                }
            };

            if !needs_update {
                continue;
            }

            // Fetch routes for this node
            let routes: PaginatedResponse<EnvoyRoute> = self
                .client
                .get(&format!(
                    "/envoy-routes?envoy_node_id={}&status=active&per_page=100",
                    node.id
                ))
                .await?;

            info!(
                node_id = %node.node_id,
                version = node.config_version,
                routes = routes.data.len(),
                "Building xDS config"
            );

            // Build xDS resources
            let mut clusters = Vec::new();
            let mut listeners = Vec::new();
            for route in &routes.data {
                if let Some(cluster) = xds_resources::build_cluster(route) {
                    clusters.push(cluster);
                    listeners.push(xds_resources::build_listener(route));
                }
            }

            // Update state
            {
                let mut state = self.state.write().await;
                state.configs.insert(
                    node.node_id.clone(),
                    NodeConfig {
                        version: node.config_version.to_string(),
                        clusters,
                        listeners,
                    },
                );
            }

            updated += 1;
        }

        // Remove configs for nodes no longer active
        {
            let active_ids: Vec<&str> = nodes.data.iter().map(|n| n.node_id.as_str()).collect();
            let mut state = self.state.write().await;
            state.configs.retain(|id, _| active_ids.contains(&id.as_str()));
        }

        if updated > 0 {
            info!(updated, "xDS config updated");
            self.notify.notify_waiters();
        }

        Ok(updated)
    }

    /// Push config for a specific node_id (CDS then LDS) to a connected client.
    async fn send_config(
        &self,
        node_id: &str,
        tx: &mpsc::Sender<Result<DiscoveryResponse, Status>>,
        nonce: &mut u64,
    ) -> Result<(), anyhow::Error> {
        let state = self.state.read().await;
        let config = state.configs.get(node_id).ok_or_else(|| {
            anyhow::anyhow!("no config for node_id '{}'", node_id)
        })?;

        // Send CDS first (clusters before listeners)
        *nonce += 1;
        let cds_response = DiscoveryResponse {
            type_url: xds_resources::CLUSTER_TYPE_URL.to_string(),
            version_info: config.version.clone(),
            nonce: nonce.to_string(),
            resources: config.clusters.clone(),
            ..Default::default()
        };
        tx.send(Ok(cds_response))
            .await
            .map_err(|_| anyhow::anyhow!("xDS stream closed"))?;

        // Send LDS
        *nonce += 1;
        let lds_response = DiscoveryResponse {
            type_url: xds_resources::LISTENER_TYPE_URL.to_string(),
            version_info: config.version.clone(),
            nonce: nonce.to_string(),
            resources: config.listeners.clone(),
            ..Default::default()
        };
        tx.send(Ok(lds_response))
            .await
            .map_err(|_| anyhow::anyhow!("xDS stream closed"))?;

        Ok(())
    }
}

#[tonic::async_trait]
impl AggregatedDiscoveryService for XdsServer {
    type StreamAggregatedResourcesStream = ReceiverStream<Result<DiscoveryResponse, Status>>;
    type DeltaAggregatedResourcesStream = ReceiverStream<Result<DeltaDiscoveryResponse, Status>>;

    async fn stream_aggregated_resources(
        &self,
        request: Request<Streaming<DiscoveryRequest>>,
    ) -> Result<Response<Self::StreamAggregatedResourcesStream>, Status> {
        let mut in_stream = request.into_inner();
        let (tx, rx) = mpsc::channel(16);

        let server = self.clone();

        tokio::spawn(async move {
            // Wait for initial request from Envoy
            let first_req = match in_stream.message().await {
                Ok(Some(req)) => req,
                Ok(None) => return,
                Err(e) => {
                    warn!("xDS stream error on first request: {}", e);
                    return;
                }
            };

            let node_id = first_req
                .node
                .as_ref()
                .map(|n| n.id.clone())
                .unwrap_or_default();

            if node_id.is_empty() {
                warn!("xDS client connected without node.id, closing stream");
                return;
            }

            info!(node_id = %node_id, "xDS client connected");

            // Send initial config (may fail if node not yet known — retry on next poll)
            let mut nonce: u64 = 0;
            match server.send_config(&node_id, &tx, &mut nonce).await {
                Ok(()) => {}
                Err(e) => {
                    warn!(node_id = %node_id, "No config available yet, will push on next poll: {:#}", e);
                }
            }

            // Main loop: wait for config changes or incoming ACK/NACK
            let notify = server.notify.clone();
            loop {
                tokio::select! {
                    _ = notify.notified() => {
                        match server.send_config(&node_id, &tx, &mut nonce).await {
                            Ok(()) => {}
                            Err(e) => {
                                warn!(node_id = %node_id, "Failed to push xDS config: {:#}", e);
                            }
                        }
                    }
                    msg = in_stream.message() => {
                        match msg {
                            Ok(Some(req)) => {
                                if let Some(ref err) = req.error_detail {
                                    warn!(
                                        node_id = %node_id,
                                        type_url = %req.type_url,
                                        nonce = %req.response_nonce,
                                        error = %err.message,
                                        "xDS NACK"
                                    );
                                } else {
                                    info!(
                                        node_id = %node_id,
                                        type_url = %req.type_url,
                                        version = %req.version_info,
                                        "xDS ACK"
                                    );
                                }
                            }
                            Ok(None) => {
                                info!(node_id = %node_id, "xDS client disconnected");
                                break;
                            }
                            Err(e) => {
                                warn!(node_id = %node_id, "xDS stream error: {}", e);
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn delta_aggregated_resources(
        &self,
        _request: Request<Streaming<DeltaDiscoveryRequest>>,
    ) -> Result<Response<Self::DeltaAggregatedResourcesStream>, Status> {
        Err(Status::unimplemented("Delta xDS not supported"))
    }
}

/// Start the xDS gRPC server on the given port.
pub async fn serve_xds(port: u16, server: XdsServer) -> Result<()> {
    let addr: std::net::SocketAddr = format!("0.0.0.0:{}", port).parse()?;
    info!(addr = %addr, "xDS gRPC server starting");

    tonic::transport::Server::builder()
        .add_service(AggregatedDiscoveryServiceServer::new(server))
        .serve(addr)
        .await?;

    Ok(())
}
