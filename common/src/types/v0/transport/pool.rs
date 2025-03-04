use super::*;

use crate::{
    types::v0::store::pool::{PoolLabel, PoolSpec},
    IntoOption,
};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, fmt::Debug, ops::Deref};
use strum_macros::{EnumString, ToString};

/// Pool Service
/// Get all the pools from specific node or None for all nodes
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct GetPools {
    /// Filter request
    pub filter: Filter,
}

/// Status of the Pool
#[derive(Serialize, Deserialize, Debug, Clone, EnumString, ToString, Eq, PartialEq)]
pub enum PoolStatus {
    /// unknown state
    Unknown = 0,
    /// the pool is in normal working order
    Online = 1,
    /// the pool has experienced a failure but can still function
    Degraded = 2,
    /// the pool is completely inaccessible
    Faulted = 3,
}

impl Default for PoolStatus {
    fn default() -> Self {
        Self::Unknown
    }
}
impl From<i32> for PoolStatus {
    fn from(src: i32) -> Self {
        match src {
            1 => Self::Online,
            2 => Self::Degraded,
            3 => Self::Faulted,
            _ => Self::Unknown,
        }
    }
}
impl From<PoolStatus> for models::PoolStatus {
    fn from(src: PoolStatus) -> Self {
        match src {
            PoolStatus::Unknown => Self::Unknown,
            PoolStatus::Online => Self::Online,
            PoolStatus::Degraded => Self::Degraded,
            PoolStatus::Faulted => Self::Faulted,
        }
    }
}

/// Pool information
#[derive(Serialize, Deserialize, Default, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PoolState {
    /// id of the io-engine instance
    pub node: NodeId,
    /// id of the pool
    pub id: PoolId,
    /// absolute disk paths claimed by the pool
    pub disks: Vec<PoolDeviceUri>,
    /// current state of the pool
    pub status: PoolStatus,
    /// size of the pool in bytes
    pub capacity: u64,
    /// used bytes from the pool
    pub used: u64,
}

impl From<PoolState> for models::PoolState {
    fn from(src: PoolState) -> Self {
        Self::new(
            src.capacity,
            src.disks,
            src.id,
            src.node,
            src.status,
            src.used,
        )
    }
}

rpc_impl_string_id!(PoolId, "ID of a pool");
rpc_impl_string_uuid!(PoolUuid, "UUID of a pool");

// online > degraded > unknown/faulted
impl PartialOrd for PoolStatus {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self {
            PoolStatus::Unknown => match other {
                PoolStatus::Unknown => None,
                PoolStatus::Online => Some(Ordering::Less),
                PoolStatus::Degraded => Some(Ordering::Less),
                PoolStatus::Faulted => None,
            },
            PoolStatus::Online => match other {
                PoolStatus::Unknown => Some(Ordering::Greater),
                PoolStatus::Online => Some(Ordering::Equal),
                PoolStatus::Degraded => Some(Ordering::Greater),
                PoolStatus::Faulted => Some(Ordering::Greater),
            },
            PoolStatus::Degraded => match other {
                PoolStatus::Unknown => Some(Ordering::Greater),
                PoolStatus::Online => Some(Ordering::Less),
                PoolStatus::Degraded => Some(Ordering::Equal),
                PoolStatus::Faulted => Some(Ordering::Greater),
            },
            PoolStatus::Faulted => match other {
                PoolStatus::Unknown => None,
                PoolStatus::Online => Some(Ordering::Less),
                PoolStatus::Degraded => Some(Ordering::Less),
                PoolStatus::Faulted => Some(Ordering::Equal),
            },
        }
    }
}

/// A Storage Pool
/// It may have a spec which is the specification provided by the creator
/// It may have a state if such state is retrieved from a storage node
#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Pool {
    /// pool identification
    id: PoolId,
    /// Desired specification of the pool.
    spec: Option<PoolSpec>,
    /// Runtime state of the pool.
    state: Option<PoolState>,
}

impl Pool {
    /// Construct a new pool with spec and state
    pub fn new(spec: PoolSpec, state: PoolState) -> Self {
        Self {
            id: spec.id.clone(),
            spec: Some(spec),
            state: Some(state),
        }
    }
    /// Construct a new pool with spec but no state
    pub fn from_spec(spec: PoolSpec) -> Self {
        Self {
            id: spec.id.clone(),
            spec: Some(spec),
            state: None,
        }
    }
    /// Construct a new pool with optional spec and state
    pub fn from_state(state: PoolState, spec: Option<PoolSpec>) -> Self {
        Self {
            id: state.id.clone(),
            spec,
            state: Some(state),
        }
    }
    /// Try to construct a new pool from spec and state
    pub fn try_new(spec: Option<PoolSpec>, state: Option<PoolState>) -> Option<Self> {
        match (spec, state) {
            (Some(spec), Some(state)) => Some(Self::new(spec, state)),
            (Some(spec), None) => Some(Self::from_spec(spec)),
            (None, Some(state)) => Some(Self::from_state(state, None)),
            _ => None,
        }
    }
    /// Get the pool spec.
    pub fn spec(&self) -> Option<PoolSpec> {
        self.spec.clone()
    }
    /// Get the pool identification.
    pub fn id(&self) -> &PoolId {
        &self.id
    }
    /// Get the pool state.
    pub fn state(&self) -> Option<PoolState> {
        self.state.clone()
    }
    /// Get the node identification
    pub fn node(&self) -> NodeId {
        match &self.spec {
            // guaranteed that at either spec or state are defined
            // todo: use enum derivation
            None => self.state.as_ref().unwrap().node.clone(),
            Some(spec) => spec.node.clone(),
        }
    }
}

impl From<Pool> for models::Pool {
    fn from(src: Pool) -> Self {
        models::Pool::new_all(src.id, src.spec.into_opt(), src.state.into_opt())
    }
}

/// Pool device URI
/// Can be specified in the form of a file path or a URI
/// eg: /dev/sda, aio:///dev/sda, malloc:///disk?size_mb=100
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct PoolDeviceUri(String);
impl Deref for PoolDeviceUri {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Default for PoolDeviceUri {
    fn default() -> Self {
        Self("malloc:///disk?size_mb=100".into())
    }
}
impl From<&str> for PoolDeviceUri {
    fn from(device: &str) -> Self {
        Self(device.to_string())
    }
}
impl From<&String> for PoolDeviceUri {
    fn from(device: &String) -> Self {
        Self(device.clone())
    }
}
impl From<String> for PoolDeviceUri {
    fn from(device: String) -> Self {
        Self(device)
    }
}
impl ToString for PoolDeviceUri {
    fn to_string(&self) -> String {
        self.deref().to_string()
    }
}
impl From<PoolDeviceUri> for String {
    fn from(device: PoolDeviceUri) -> Self {
        device.to_string()
    }
}

/// Create Pool Request
#[derive(Serialize, Deserialize, Default, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreatePool {
    /// id of the io-engine instance
    pub node: NodeId,
    /// id of the pool
    pub id: PoolId,
    /// disk device paths or URIs to be claimed by the pool
    pub disks: Vec<PoolDeviceUri>,
    /// labels to be set on the pool
    pub labels: Option<PoolLabel>,
}

impl CreatePool {
    /// Create new `Self` from the given parameters
    pub fn new(
        node: &NodeId,
        id: &PoolId,
        disks: &[PoolDeviceUri],
        labels: &Option<PoolLabel>,
    ) -> Self {
        Self {
            node: node.clone(),
            id: id.clone(),
            disks: disks.to_vec(),
            labels: labels.clone(),
        }
    }
}

/// Destroy Pool Request
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DestroyPool {
    /// id of the io-engine instance
    pub node: NodeId,
    /// id of the pool
    pub id: PoolId,
}
