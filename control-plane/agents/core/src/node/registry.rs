use crate::core::{registry::Registry, wrapper::NodeWrapper};
use common::errors::SvcError;
use common_lib::types::v0::transport::{DrainState, NodeId, NodeState, Register};

use std::sync::Arc;
use tokio::sync::RwLock;

impl Registry {
    /// Get all node wrappers
    pub(crate) async fn get_node_wrappers(&self) -> Vec<Arc<RwLock<NodeWrapper>>> {
        let nodes = self.nodes().read().await;
        nodes.values().cloned().collect()
    }

    /// Get all node states
    pub(crate) async fn get_node_states(&self) -> Vec<NodeState> {
        let nodes = self.nodes().read().await;
        let mut nodes_vec = vec![];
        for node in nodes.values() {
            nodes_vec.push(node.read().await.node_state().clone());
        }
        nodes_vec
    }

    /// Get node wrapper by its `NodeId`
    pub(crate) async fn get_node_wrapper(
        &self,
        node_id: &NodeId,
    ) -> Result<Arc<RwLock<NodeWrapper>>, SvcError> {
        match self.nodes().read().await.get(node_id).cloned() {
            None => {
                if self.specs().get_node(node_id).is_ok() {
                    Err(SvcError::NodeNotOnline {
                        node: node_id.to_owned(),
                    })
                } else {
                    Err(SvcError::NodeNotFound {
                        node_id: node_id.to_owned(),
                    })
                }
            }
            Some(node) => Ok(node),
        }
    }

    /// Get node state by its `NodeId`
    pub(crate) async fn get_node_state(&self, node_id: &NodeId) -> Result<NodeState, SvcError> {
        match self.nodes().read().await.get(node_id).cloned() {
            None => {
                if self.specs().get_node(node_id).is_ok() {
                    Err(SvcError::NodeNotOnline {
                        node: node_id.to_owned(),
                    })
                } else {
                    Err(SvcError::NodeNotFound {
                        node_id: node_id.to_owned(),
                    })
                }
            }
            Some(node) => {
                let drains = self.specs().get_node(node_id).unwrap().drain_labels();
                let nexuses = self.get_node_nexuses(node_id).await;
                let mut drain_state = DrainState::NotDraining;
                if !drains.is_empty() {
                    if nexuses.unwrap().is_empty() {
                        drain_state = DrainState::Drained;
                    } else {
                        drain_state = DrainState::Draining;
                    }
                }
                let mut copy_node_state = node.read().await.node_state().clone();
                copy_node_state.drain_state = drain_state;
                Ok(copy_node_state)
            }
        }
    }

    /// Register new NodeSpec for the given `Register` Request
    pub(super) async fn register_node_spec(&self, request: &Register) {
        if self.config().node_registration().automatic() {
            self.specs().register_node(self, request).await.ok();
        }
    }
}
