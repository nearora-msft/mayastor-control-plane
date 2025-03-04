#![allow(clippy::field_reassign_with_default)]

use super::super::RestClient;
use std::convert::{TryFrom, TryInto};

pub use common_lib::{
    transport_api,
    types::v0::{
        openapi::{apis, apis::actix_server::RestError, models, tower::client},
        store::pool::PoolLabel,
        transport::{
            AddNexusChild, BlockDevice, Child, ChildUri, CreateNexus, CreatePool, CreateReplica,
            CreateVolume, DestroyNexus, DestroyPool, DestroyReplica, DestroyVolume, Filter,
            GetBlockDevices, JsonGrpcRequest, Nexus, NexusId, NexusShareProtocol, Node, NodeId,
            Pool, PoolDeviceUri, PoolId, Protocol, RemoveNexusChild, Replica, ReplicaId,
            ReplicaShareProtocol, ShareNexus, ShareReplica, Specs, Topology, UnshareNexus,
            UnshareReplica, VolumeId, VolumeLabels, VolumePolicy, Watch, WatchCallback,
            WatchResourceId,
        },
    },
};

use common_lib::{IntoOption, IntoVec};
pub use models::rest_json_error::Kind as RestJsonErrorKind;

use common_lib::{
    transport_api::ReplyError,
    types::v0::{
        openapi::models::RestJsonError,
        transport::{HostNqn, HostNqnParseError},
    },
};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Create Replica Body JSON.
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct CreateReplicaBody {
    /// Size of the replica in bytes.
    pub size: u64,
    /// Thin provisioning.
    pub thin: bool,
    /// Protocol to expose the replica over.
    pub share: Protocol,
    /// Host nqn's allowed to connect to the target.
    pub allowed_hosts: Vec<HostNqn>,
}

impl TryFrom<models::CreateReplicaBody> for CreateReplicaBody {
    type Error = RestError<RestJsonError>;
    fn try_from(src: models::CreateReplicaBody) -> Result<Self, Self::Error> {
        Ok(Self {
            size: src.size as u64,
            thin: src.thin,
            share: match src.share {
                None => Protocol::None,
                Some(models::ReplicaShareProtocol::Nvmf) => Protocol::Nvmf,
            },
            allowed_hosts: src
                .allowed_hosts
                .unwrap_or_default()
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, HostNqnParseError>>()
                .map_err(ReplyError::from)?,
        })
    }
}

/// Create Pool Body JSON
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct CreatePoolBody {
    /// disk device paths or URIs to be claimed by the pool
    pub disks: Vec<PoolDeviceUri>,
    /// labels to be set on the pool
    pub labels: Option<PoolLabel>,
}
impl From<models::CreatePoolBody> for CreatePoolBody {
    fn from(src: models::CreatePoolBody) -> Self {
        Self {
            disks: src.disks.iter().cloned().map(From::from).collect(),
            labels: src.labels,
        }
    }
}
impl From<CreatePool> for CreatePoolBody {
    fn from(create: CreatePool) -> Self {
        CreatePoolBody {
            disks: create.disks,
            labels: create.labels,
        }
    }
}
impl CreatePoolBody {
    /// Convert into rpc request type.
    pub fn to_request(&self, node_id: NodeId, pool_id: PoolId) -> CreatePool {
        CreatePool {
            node: node_id,
            id: pool_id,
            disks: self.disks.clone(),
            labels: self.labels.clone(),
        }
    }
}

impl From<CreateReplica> for CreateReplicaBody {
    fn from(create: CreateReplica) -> Self {
        CreateReplicaBody {
            size: create.size,
            thin: create.thin,
            share: create.share,
            allowed_hosts: create.allowed_hosts.into_vec(),
        }
    }
}
impl CreateReplicaBody {
    /// Convert into rpc request type.
    pub fn to_request(self, node_id: NodeId, pool_id: PoolId, uuid: ReplicaId) -> CreateReplica {
        CreateReplica {
            node: node_id,
            name: None,
            uuid,
            pool_id,
            pool_uuid: None,
            size: self.size,
            thin: self.thin,
            share: self.share,
            managed: false,
            owners: Default::default(),
            allowed_hosts: self.allowed_hosts,
        }
    }
}

/// Create Nexus Body JSON
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct CreateNexusBody {
    /// size of the device in bytes
    pub size: u64,
    /// replica can be iscsi and nvmf remote targets or a local spdk bdev
    /// (i.e. bdev:///name-of-the-bdev).
    ///
    /// uris to the targets we connect to
    pub children: Vec<ChildUri>,
}
impl From<CreateNexus> for CreateNexusBody {
    fn from(create: CreateNexus) -> Self {
        Self {
            size: create.size,
            children: create.children.into_vec(),
        }
    }
}
impl From<models::CreateNexusBody> for CreateNexusBody {
    fn from(src: models::CreateNexusBody) -> Self {
        Self {
            size: src.size as u64,
            children: src.children.into_iter().map(From::from).collect(),
        }
    }
}
impl CreateNexusBody {
    /// Convert into rpc request type.
    pub fn to_request(&self, node_id: NodeId, nexus_id: NexusId) -> CreateNexus {
        CreateNexus {
            node: node_id,
            uuid: nexus_id,
            size: self.size,
            children: self.children.clone().into_vec(),
            managed: false,
            owner: None,
            config: None,
        }
    }
}

/// Create Volume Body JSON
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct CreateVolumeBody {
    /// size of the volume in bytes
    pub size: u64,
    /// number of storage replicas
    pub replicas: u64,
    /// Volume policy used to determine if and how to replace a replica
    pub policy: VolumePolicy,
    /// Volume topology used to determine how to place/distribute the data
    pub topology: Option<Topology>,
    /// Volume labels, used ot store custom volume information
    pub labels: Option<VolumeLabels>,
    /// Flag indicating whether the volume should be thin provisioned
    pub thin: bool,
}
impl From<models::CreateVolumeBody> for CreateVolumeBody {
    fn from(src: models::CreateVolumeBody) -> Self {
        Self {
            size: src.size as u64,
            replicas: src.replicas as u64,
            policy: src.policy.into(),
            topology: src.topology.into_opt(),
            labels: src.labels,
            thin: src.thin,
        }
    }
}
impl From<CreateVolume> for CreateVolumeBody {
    fn from(create: CreateVolume) -> Self {
        CreateVolumeBody {
            size: create.size,
            replicas: create.replicas,
            policy: create.policy,
            topology: create.topology,
            labels: create.labels,
            thin: create.thin,
        }
    }
}
impl CreateVolumeBody {
    /// Convert into rpc request type.
    pub fn to_create_volume(&self, volume_id: VolumeId) -> CreateVolume {
        CreateVolume {
            uuid: volume_id,
            size: self.size,
            replicas: self.replicas,
            policy: self.policy.clone(),
            topology: self.topology.clone(),
            labels: self.labels.clone(),
            thin: self.thin,
        }
    }
}

impl RestClient {
    /// Get Autogenerated Openapi client v0
    pub fn v00(&self) -> client::direct::ApiClient {
        self.openapi_client_v0.clone()
    }
}
