use crate::{
    operations::{Cordoning, Drain, Get, List},
    resources::{
        utils,
        utils::{print_table, CreateRows, GetHeaderRow, OutputFormat},
        NodeId,
    },
    rest_wrapper::RestClient,
};
use async_trait::async_trait;
use openapi::models::NodeSpec;
use prettytable::{Cell, Row};
use serde::Serialize;

#[derive(Debug, Clone, clap::Args)]
/// Arguments used when getting a node.
pub struct GetNodeArgs {
    /// Id of the node
    node_id: NodeId,
    #[clap(long)]
    /// Shows the cordon labels associated with the node
    show_cordon_labels: bool,
}

impl GetNodeArgs {
    /// Return the node ID.
    pub fn node_id(&self) -> NodeId {
        self.node_id.clone()
    }

    /// Return whether or not we should show the cordon labels.
    pub fn show_cordon_labels(&self) -> bool {
        self.show_cordon_labels
    }
}

/// Nodes resource.
#[derive(clap::Args, Debug)]
pub struct Nodes {}

// CreateRows being trait for Node would create the rows from the list of
// Nodes returned from REST call.
impl CreateRows for openapi::models::Node {
    fn create_rows(&self) -> Vec<Row> {
        let spec = self.spec.clone().unwrap_or_default();
        // In case the state is not coming as filled, either due to node offline, fill in
        // spec data and mark the status as Unknown.
        let state = self.state.clone().unwrap_or(openapi::models::NodeState {
            id: spec.id,
            grpc_endpoint: spec.grpc_endpoint,
            status: openapi::models::NodeStatus::Unknown,
        });
        let rows = vec![row![
            self.id,
            state.grpc_endpoint,
            state.status,
            !(spec.cordon_labels.is_empty() && spec.drain_labels.is_empty()),
        ]];
        rows
    }
}

// GetHeaderRow being trait for Node would return the Header Row for
// Node.
impl GetHeaderRow for openapi::models::Node {
    fn get_header_row(&self) -> Row {
        (&*utils::NODE_HEADERS).clone()
    }
}

#[async_trait(?Send)]
impl List for Nodes {
    async fn list(output: &utils::OutputFormat) {
        match RestClient::client().nodes_api().get_nodes().await {
            Ok(nodes) => {
                // Print table, json or yaml based on output format.
                utils::print_table(output, nodes.into_body());
            }
            Err(e) => {
                println!("Failed to list nodes. Error {}", e)
            }
        }
    }
}

/// Node resource.
#[derive(clap::Args, Debug)]
pub struct Node {}

#[async_trait(?Send)]
impl Get for Node {
    type ID = NodeId;
    async fn get(id: &Self::ID, output: &utils::OutputFormat) {
        match RestClient::client().nodes_api().get_node(id).await {
            Ok(node) => {
                // Print table, json or yaml based on output format.
                let node_display = NodeDisplay::new(node.into_body(), NodeDisplayFormat::Default);
                print_table(output, node_display);
            }
            Err(e) => {
                println!("Failed to get node {}. Error {}", id, e)
            }
        }
    }
}

#[async_trait(?Send)]
impl Cordoning for Node {
    type ID = NodeId;
    async fn cordon(id: &Self::ID, label: &str, output: &OutputFormat) {
        match RestClient::client()
            .nodes_api()
            .put_node_cordon(id, label)
            .await
        {
            Ok(node) => match output {
                OutputFormat::Yaml | OutputFormat::Json => {
                    // Print json or yaml based on output format.
                    utils::print_table(output, node.into_body());
                }
                OutputFormat::None => {
                    // In case the output format is not specified, show a success message.
                    println!("Node {} cordoned successfully", id)
                }
            },
            Err(e) => {
                println!("Failed to cordon node {}. Error {}", id, e)
            }
        }
    }

    async fn uncordon(id: &Self::ID, label: &str, output: &OutputFormat) {
        match RestClient::client()
            .nodes_api()
            .delete_node_cordon(id, label)
            .await
        {
            Ok(node) => match output {
                OutputFormat::Yaml | OutputFormat::Json => {
                    // Print json or yaml based on output format.
                    utils::print_table(output, node.into_body());
                }
                OutputFormat::None => {
                    // In case the output format is not specified, show a success message.
                    let cordon_labels = node
                        .clone()
                        .into_body()
                        .spec
                        .map(|node_spec| node_spec.cordon_labels)
                        .unwrap_or_default();

                    let drain_labels = node
                        .into_body()
                        .spec
                        .map(|node_spec| node_spec.drain_labels)
                        .unwrap_or_default();
                    let labels = [cordon_labels, drain_labels].concat();
                    if labels.is_empty() {
                        println!("Node {} successfully uncordoned", id);
                    } else {
                        println!(
                            "Cordon label successfully removed. Remaining cordon labels {:?}",
                            labels,
                        );
                    }
                }
            },
            Err(e) => {
                println!("Failed to uncordon node {}. Error {}", id, e)
            }
        }
    }

    async fn get_node_with_cordon_labels(id: &Self::ID, output: &OutputFormat) {
        match RestClient::client().nodes_api().get_node(id).await {
            Ok(node) => {
                let node_display =
                    NodeDisplay::new(node.into_body(), NodeDisplayFormat::CordonLabels);
                print_table(output, node_display);
            }
            Err(e) => {
                println!("Failed to get node {}. Error {}", id, e)
            }
        }
    }
}

/// Display format options for a `Node` object.
#[derive(Debug)]
enum NodeDisplayFormat {
    Default,
    CordonLabels,
    Drain,
}

/// The NodeDisply structure is responsible for controlling the display formatting of Node objects.
/// `#[serde(flatten)]` and `#[serde(skip)]` attributes are used to ensure that when the object is
/// serialised, only the `inner` object is represented.
#[derive(Serialize, Debug)]
struct NodeDisplay {
    #[serde(flatten)]
    inner: openapi::models::Node,
    #[serde(skip)]
    format: NodeDisplayFormat,
}

impl NodeDisplay {
    /// Create a new `NodeDisplay` instance.
    pub(crate) fn new(node: openapi::models::Node, format: NodeDisplayFormat) -> Self {
        Self {
            inner: node,
            format,
        }
    }
}

// Create the rows required for a `NodeDisplay` object. Nodes returned from REST call.
impl CreateRows for NodeDisplay {
    fn create_rows(&self) -> Vec<Row> {
        match self.format {
            NodeDisplayFormat::Default => self.inner.create_rows(),
            NodeDisplayFormat::CordonLabels => {
                let mut rows = self.inner.create_rows();
                let mut cordon_labels_string = self
                    .inner
                    .spec
                    .as_ref()
                    .unwrap_or(&NodeSpec::default())
                    .cordon_labels
                    .join(", ");
                let drain_labels_string = self
                    .inner
                    .spec
                    .as_ref()
                    .unwrap_or(&NodeSpec::default())
                    .drain_labels
                    .join(", ");
                if !cordon_labels_string.is_empty() && !drain_labels_string.is_empty() {
                    cordon_labels_string += ", ";
                }
                cordon_labels_string += &drain_labels_string;
                // Add the cordon labels to each row.
                rows.iter_mut()
                    .for_each(|row| row.add_cell(Cell::new(&cordon_labels_string)));

                rows
            }
            NodeDisplayFormat::Drain => {
                let mut rows = self.inner.create_rows();
                let drain_state_string = String::from("not draining"); // temporary - will get this from node state
                let drain_labels_string = self
                    .inner
                    .spec
                    .as_ref()
                    .unwrap_or(&NodeSpec::default())
                    .drain_labels
                    .join(", ");
                // Add the drain labels to each row.
                rows.iter_mut().for_each(|row| {
                    row.add_cell(Cell::new(&drain_state_string));
                    row.add_cell(Cell::new(&drain_labels_string));
                });
                rows
            }
        }
    }
}

// Create the header for a `NodeDisplay` object.
impl GetHeaderRow for NodeDisplay {
    fn get_header_row(&self) -> Row {
        let mut header = (&*utils::NODE_HEADERS).clone();
        match self.format {
            NodeDisplayFormat::Default => header,
            NodeDisplayFormat::CordonLabels => {
                header.extend(vec!["CORDON LABELS"]);
                header
            }
            NodeDisplayFormat::Drain => {
                header.extend(vec!["DRAIN STATE"]);
                header.extend(vec!["DRAIN LABELS"]);
                header
            }
        }
    }
}

#[async_trait(?Send)]
impl Drain for Node {
    type ID = NodeId;
    async fn drain(id: &Self::ID, label: String, output: &utils::OutputFormat) {
        // loop this call until the put_node_drain call returns
        match RestClient::client()
            .nodes_api()
            .put_node_drain(id, &label)
            .await
        {
            Ok(node) => {
                // Print table, json or yaml based on output format.
                utils::print_table(output, node.into_body());
            }
            Err(e) => {
                println!("Failed to get node {}. Error {}", id, e)
            }
        }
    }
    async fn get_node_drain(id: &Self::ID, output: &OutputFormat) {
        match RestClient::client().nodes_api().get_node(id).await {
            Ok(node) => {
                let node_display = NodeDisplay::new(node.into_body(), NodeDisplayFormat::Drain);
                print_table(output, node_display);
            }
            Err(e) => {
                println!("Failed to get node {}. Error {}", id, e)
            }
        }
    }
}
