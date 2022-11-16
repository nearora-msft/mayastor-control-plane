use crate::{
    detector::{NvmeController, NvmePathCache},
    path_provider::get_nvme_path_buf,
};
use agents::errors::SvcError;
use common_lib::transport_api::{ErrorChain, ReplyError, ResourceKind};
use grpc::{
    context::Context,
    operations::ha_node::{
        server::NodeAgentServer,
        traits::{NodeAgentOperations, ReplacePathInfo},
    },
};
use http::Uri;
use nvmeadm::{
    nvmf_discovery::{ConnectArgs, ConnectArgsBuilder},
    nvmf_subsystem::{NvmeSubsystems, Subsystem},
};
use std::{net::SocketAddr, sync::Arc};
use tokio::time::{sleep, Duration};
use common_lib::types::v0::store::volume::NvmfParameters;
use utils::NVME_TARGET_NQN_PREFIX;

/// Common error source name for all gRPC errors in HA Node agent.
const HA_AGENT_ERR_SOURCE: &str = "HA Node agent gRPC server";

/// NVMe subsystem refresh interval when monitoring its state, in milliseconds.
const SUBSYSTEM_STATE_REFRESH_INTERVAL_MS: u64 = 500;

/// High-level object that represents HA Node agent gRPC server.
pub(crate) struct NodeAgentApiServer {
    endpoint: SocketAddr,
    path_cache: NvmePathCache,
}

impl NodeAgentApiServer {
    /// Returns a new `Self` with the given parameters.
    pub(crate) fn new(endpoint: SocketAddr, path_cache: NvmePathCache) -> Self {
        Self {
            endpoint,
            path_cache,
        }
    }

    /// Runs this server as a future until a shutdown signal is received.
    pub(crate) async fn serve(&self) -> Result<(), agents::ServiceError> {
        let r = NodeAgentServer::new(Arc::new(NodeAgentSvc::new(self.path_cache.clone())));
        tracing::info!("Starting gRPC server at {:?}", self.endpoint);
        agents::Service::builder()
            .with_service(r.into_grpc_server())
            .run_err(self.endpoint)
            .await
    }
}

/// The gRPC server implementation for the HA Node agent.
struct NodeAgentSvc {
    path_cache: NvmePathCache,
}

impl NodeAgentSvc {
    /// Returns a new `Self` with the given parameters.
    pub(crate) fn new(path_cache: NvmePathCache) -> Self {
        Self { path_cache }
    }
}

/// Disconnect cached NVMe controller.
fn disconnect_controller(ctrlr: &NvmeController, new_path: String) -> Result<(), SvcError> {
    let parsed_path = parse_uri(new_path.as_str())?;

    match get_nvme_path_buf(&ctrlr.path) {
        Some(pbuf) => {
            let subsystem = Subsystem::new(pbuf.as_path()).map_err(|_| SvcError::Internal {
                details: "Failed to get NVMe subsystem for controller".to_string(),
            })?;

            if subsystem.address == SubsystemAddr::new(parsed_path.host(), parsed_path.port()) {
                tracing::info!(
                path=%ctrlr.path,
                "Not disconnecting same NVMe controller");

                Ok(())
            } else {
                tracing::info!(
                    path=%ctrlr.path,
                    "Disconnecting NVMe controller"
                );

                subsystem.disconnect().map_err(|e| SvcError::Internal {
                    details: format!(
                        "Failed to disconnect NVMe controller {}: {:?}",
                        ctrlr.path, e,
                    ),
                })
            }
        }
        None => {
            tracing::error!(
                path=%ctrlr.path,
                "Failed to get system path for controller"
            );

            Err(SvcError::Internal {
                details: "Failed to get system path for controller".to_string(),
            })
        }
    }
}

impl NodeAgentSvc {
    /// Translate new path URI into connection arguments for Subsystem connect API.
    fn get_nvmf_connection_args(&self, new_path: &str, nvmf_parameters: NvmfParameters) -> Option<ConnectArgs> {
        let parsed_path = parse_uri(new_path).ok()?;

        // Check NQN of the subsystem to make sure it belongs to the product.
        if !parsed_path.nqn().starts_with(NVME_TARGET_NQN_PREFIX) {
            return None;
        }

        ConnectArgsBuilder::default()
            .traddr(parsed_path.host())
            .trsvcid(parsed_path.port())
            .nqn(parsed_path.nqn())
            .ctrl_loss_tmo(Some(nvmf_parameters.ctlr_loss_timeout))
            .reconnect_delay(Some(nvmf_parameters.ctlr_loss_timeout))
            .build()
            .ok()
    }

    /// Connect NVMe controller. Wait till the controller is fully connected.
    async fn connect_controller(&self, new_path: String, nqn: String, nvmf_parameters: NvmfParameters) -> Result<(), SvcError> {
        let connect_args = match self.get_nvmf_connection_args(&new_path, nvmf_parameters) {
            Some(ca) => ca,
            None => return Err(SvcError::InvalidArguments {}),
        };

        tracing::info!(new_path, "Connecting to NVMe target");

        // Open connection to the new nexus: ANA will automatically create
        // the second path and add it as an alternative for the first broken one,
        // which immediately resumes all stalled I/O

        let parsed_uri = parse_uri(new_path.as_str())?;
        let mut subsystem = match get_subsystem(&parsed_uri) {
            Ok(subsystem) => subsystem,
            Err(_) => match connect_args.connect() {
                Ok(subsystem) => {
                    tracing::info!(new_path, "Successfully connected to NVMe target");
                    subsystem
                }
                Err(error) => {
                    tracing::error!(
                        new_path,
                        error=%error,
                        "Failed to connect to new NVMe target"
                    );
                    let nvme_err = format!(
                        "Failed to connect to new NVMe target: {}, new path: {}, nqn: {}",
                        error.full_string(),
                        new_path,
                        nqn
                    );
                    return Err(SvcError::Internal { details: nvme_err });
                }
            },
        };

        // Wait till new controller is fully connected before completing the call.
        // Straight after connection subsystem transitions into 'new' state, then
        // proceeds to 'connecting' till the connection is fully established,
        // so wait a bit before checking the state.
        loop {
            sleep(Duration::from_millis(SUBSYSTEM_STATE_REFRESH_INTERVAL_MS)).await;

            // Refresh subsystem to get the latest state.
            if let Err(error) = subsystem.sync() {
                tracing::error!(
                    new_path,
                    error=%error,
                    "Failed to synchronize NVMe subsystem state"
                );
                // Just log error and exit, since such a situation can take
                // place when the path is explicitly removed by user.
                break;
            }

            // TODO: consider max retries to prevent controller from
            // not reaching 'live' state.
            match subsystem.state.as_str() {
                "connecting" | "new" => {
                    tracing::info!(
                        new_path,
                        state = "connecting",
                        "New NVMe path is not ready to serve I/O, waiting."
                    );
                }
                "live" => {
                    tracing::info!(new_path, "New NVMe path is ready to serve I/O");
                    break;
                }
                _ => {
                    tracing::error!(
                        new_path,
                        state = subsystem.state.as_str(),
                        "New NVMe path is in incorrect state"
                    );
                    return Err(SvcError::Internal {
                        details: format!(
                            "New NVMe path is in incorrect state: {}",
                            subsystem.state.as_str()
                        ),
                    });
                }
            }
        }

        Ok(())
    }
}

#[tonic::async_trait]
impl NodeAgentOperations for NodeAgentSvc {
    async fn replace_path(
        &self,
        request: &dyn ReplacePathInfo,
        _context: Option<Context>,
    ) -> Result<(), ReplyError> {
        tracing::info!("Replacing failed NVMe path: {:?}", request);
        // Lookup NVMe controller whose path has failed.
        let ctrlr = self
            .path_cache
            .lookup_controller(request.target_nqn())
            .await
            .map_err(|_| {
                ReplyError::failed_precondition(
                    ResourceKind::Nexus,
                    HA_AGENT_ERR_SOURCE.to_string(),
                    "Failed to lookup controller".to_string(),
                )
            })?;

        // Step 1: populate an additional healthy path to target NQN in addition to
        // existing failed path. Once this additional path is created, client I/O
        // automatically resumes.
        self.connect_controller(request.new_path(), request.target_nqn(), request.nvmf_parameters())
            .await?;

        // Step 2: disconnect broken path to leave the only new healthy path.
        // Note that errors under disconnection are not critical, since the second I/O
        // path has been successfully created, so having the first failed path in addition
        // to the second healthy one is OK: just display a warning and proceed as if
        // the call has completed successfully.
        if let Err(error) = disconnect_controller(&ctrlr, request.new_path()) {
            tracing::warn!(
                uri=%request.new_path(),
                error=%error,
                "Failed to disconnect failed path"
            );
        };
        Ok(())
    }
}

// Returns the host, port, nqn respectively from the path after parsing.
fn parse_uri(new_path: &str) -> Result<ParsedUri, SvcError> {
    let uri = new_path
        .parse::<Uri>()
        .map_err(|_| SvcError::InvalidArguments {})?;

    let host = uri.host().ok_or(SvcError::InvalidArguments {})?;
    let port = uri.port().ok_or(SvcError::InvalidArguments {})?;
    let nqn = &uri.path()[1 ..];
    Ok(ParsedUri::new(
        host.to_string(),
        port.to_string(),
        nqn.to_string(),
    ))
}

// Returns the particular subsystem based on the nqn and address.
fn get_subsystem(parsed_uri: &ParsedUri) -> Result<Subsystem, SvcError> {
    let nvme_subsystems = NvmeSubsystems::new().map_err(|_| SvcError::SubsystemNotFound {
        nqn: parsed_uri.nqn(),
    })?;
    for subsys in nvme_subsystems.flatten() {
        if subsys.nqn == parsed_uri.nqn()
            && subsys.address == SubsystemAddr::new(parsed_uri.host(), parsed_uri.port())
        {
            return Ok(subsys);
        }
    }
    Err(SvcError::SubsystemNotFound {
        nqn: parsed_uri.nqn(),
    })
}

struct SubsystemAddr(String);

impl SubsystemAddr {
    fn new(host: String, port: String) -> SubsystemAddr {
        SubsystemAddr(format!("traddr={},trsvcid={}", host, port))
    }
    fn string_value(&self) -> String {
        self.0.clone()
    }
}

// For SubsystemAddr == String comparisons
impl PartialEq<String> for SubsystemAddr {
    fn eq(&self, other: &String) -> bool {
        self.string_value() == *other
    }
}

// For String == SubsystemAddr comparisons
impl PartialEq<SubsystemAddr> for String {
    fn eq(&self, other: &SubsystemAddr) -> bool {
        *self == other.string_value()
    }
}

struct ParsedUri {
    host: String,
    port: String,
    nqn: String,
}

impl ParsedUri {
    fn new(host: String, port: String, nqn: String) -> ParsedUri {
        Self { host, port, nqn }
    }
    fn host(&self) -> String {
        self.host.clone()
    }
    fn port(&self) -> String {
        self.port.clone()
    }
    fn nqn(&self) -> String {
        self.nqn.clone()
    }
}
