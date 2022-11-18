use crate::{cluster_agent_client, Cli};
use common_lib::types::v0::transport::{FailedPath, ReportFailedPaths};
use grpc::operations::ha_node::traits::ClusterAgentOperations;
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    time::{sleep, Duration},
};

/// Initial size of a batch.
const DEFAULT_BATCH_SIZE: usize = 16;

/// Entity that reports failed NVMe paths to HA Cluster agent via gRPC.
/// Failed NVMe paths are always aggregated before sending, which avoids
/// many gRPC invocations for ebery individual failed path and fully utilizes
/// the ability to report multiple failed paths in one call.
#[derive(Debug)]
pub struct PathReporter {
    node_name: String,
    channel: UnboundedSender<String>,
    retransmission_period: Duration,
    aggregation_period: Duration,
}

impl PathReporter {
    pub fn new(
        node_name: String,
        retransmission_period: Duration,
        aggregation_period: Duration,
    ) -> Self {
        let (tx, rx) = unbounded_channel();

        let reporter = Self {
            channel: tx,
            node_name,
            retransmission_period,
            aggregation_period,
        };

        reporter.start(rx);
        reporter
    }

    /// Start main loop for reporter.
    fn start(&self, path_receiver: UnboundedReceiver<String>) {
        let node_name = self.node_name.clone();
        let retransmission_period = self.retransmission_period;
        let aggregation_period = self.aggregation_period;

        tracing::info!(
            "Starting path reporter (retransmission period: {:?}, aggregation period: {:?})",
            retransmission_period,
            aggregation_period,
        );

        tokio::spawn(async move {
            let mut aggregator = RequestAggregator::new(path_receiver, aggregation_period);

            loop {
                // Phase 1: wait till a path batch is available.
                let batch = match aggregator.receive_batch().await {
                    Ok(val) => val,
                    Err(_) => continue,
                };

                // Phase 2: send all aggregated paths in one shot.
                let failed_paths = batch
                    .paths()
                    .iter()
                    .map(|p| FailedPath::new(p.to_string()))
                    .collect::<Vec<FailedPath>>();
                let node_ep = Cli::args().grpc_endpoint;
                let req = ReportFailedPaths::new(node_name.clone(), failed_paths, node_ep);

                // Report all paths in a separate task, continue till transmission succeeds.
                tokio::spawn(async move {
                    let client = cluster_agent_client();

                    loop {
                        match client.report_failed_nvme_paths(&req, None).await {
                            Ok(_) => break,
                            Err(e) => {
                                tracing::error!("Failed to report failed NVMe paths: {}", e);
                                sleep(retransmission_period).await;
                            }
                        }
                    }
                });
            }
        });
    }

    pub fn report_failed_path(&self, nqn: String) {
        self.channel
            .send(nqn)
            .expect("Reporter channel disappeared");
    }
}

#[derive(Debug)]
struct PathBatch {
    pub paths: Vec<String>,
}

impl PathBatch {
    pub fn new() -> Self {
        Self {
            paths: Vec::with_capacity(DEFAULT_BATCH_SIZE),
        }
    }

    pub fn paths(&self) -> &Vec<String> {
        &self.paths
    }

    pub fn add_path(&mut self, path: String) {
        self.paths.push(path)
    }
}

/// Batched aggregator which aggregates reported paths over
/// time window and produces one batch for all paths reported
/// within this time window.
struct RequestAggregator {
    batch_receiver: UnboundedReceiver<PathBatch>,
    aggregation_period: Duration,
}

impl RequestAggregator {
    pub fn new(path_receiver: UnboundedReceiver<String>, aggregation_period: Duration) -> Self {
        let (tx, rx) = unbounded_channel();

        let receiver = Self {
            batch_receiver: rx,
            aggregation_period,
        };

        receiver.start(path_receiver, tx);
        receiver
    }

    fn start(
        &self,
        mut path_receiver: UnboundedReceiver<String>,
        batch_sender: UnboundedSender<PathBatch>,
    ) {
        let aggregation_period = self.aggregation_period;

        tokio::spawn(async move {
            // Phase 1: wait for the first path to trigger batch aggregation.
            while let Some(path) = path_receiver.recv().await {
                // Phase 2: add all subsequent reported paths to the batch.
                let mut batch = PathBatch::new();
                batch.add_path(path);

                loop {
                    tokio::select! {
                        receiver = path_receiver.recv() => {
                            match receiver {
                                Some(path) => {
                                    batch.add_path(path);
                                }
                                None => break
                            }
                        },
                        _ = sleep(aggregation_period) => {
                            break;
                        }
                    }
                }

                if batch_sender.send(batch).is_err() {
                    break;
                }
            }
        });
    }

    /// Wait till a batch of reported paths is available.
    pub async fn receive_batch(&mut self) -> anyhow::Result<PathBatch> {
        self.batch_receiver
            .recv()
            .await
            .ok_or_else(|| anyhow::Error::msg("Batch producer's channel disappeared"))
    }
}
