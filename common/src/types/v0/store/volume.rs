//! Definition of volume types that can be saved to the persistent store.

use crate::{
    types::v0::{
        openapi::models,
        store::{
            definitions::{ObjectKey, StorableObject, StorableObjectType},
            AsOperationSequencer, OperationSequence, SpecStatus, SpecTransaction,
        },
        transport::{
            self, CreateVolume, NexusId, NexusNvmfConfig, NodeId, ReplicaId, Topology, VolumeId,
            VolumeLabels, VolumePolicy, VolumeShareProtocol, VolumeStatus,
        },
    },
    IntoOption,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Key used by the store to uniquely identify a VolumeState structure.
pub struct VolumeStateKey(VolumeId);

impl From<&VolumeId> for VolumeStateKey {
    fn from(id: &VolumeId) -> Self {
        Self(id.clone())
    }
}

impl ObjectKey for VolumeStateKey {
    fn key_type(&self) -> StorableObjectType {
        StorableObjectType::VolumeState
    }

    fn key_uuid(&self) -> String {
        self.0.to_string()
    }
}

/// Volume Target (node and nexus)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct VolumeTarget {
    /// The node where front-end IO will be sent to.
    node: NodeId,
    /// The identification of the nexus where the frontend-IO will be sent to.
    nexus: NexusId,
    /// The protocol to use on the target.
    protocol: Option<VolumeShareProtocol>,
}
impl VolumeTarget {
    /// Create a new `Self` based on the given parameters.
    pub fn new(node: NodeId, nexus: NexusId, protocol: Option<VolumeShareProtocol>) -> Self {
        Self {
            node,
            nexus,
            protocol,
        }
    }
    /// Get a reference to the node identification.
    pub fn node(&self) -> &NodeId {
        &self.node
    }
    /// Get a reference to the nexus identification.
    pub fn nexus(&self) -> &NexusId {
        &self.nexus
    }
    /// Get a reference to the volume protocol.
    pub fn protocol(&self) -> Option<&VolumeShareProtocol> {
        self.protocol.as_ref()
    }
}
impl From<VolumeTarget> for models::VolumeTarget {
    fn from(src: VolumeTarget) -> Self {
        Self::new_all(src.node, src.protocol.into_opt())
    }
}

/// User specification of a volume.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct VolumeSpec {
    /// Volume Id.
    pub uuid: VolumeId,
    /// Size that the volume should be.
    pub size: u64,
    /// Volume labels.
    pub labels: Option<VolumeLabels>,
    /// Number of children the volume should have.
    pub num_replicas: u8,
    /// Status that the volume should eventually achieve.
    pub status: VolumeSpecStatus,
    /// The target where front-end IO will be sent to.
    pub target: Option<VolumeTarget>,
    /// Volume policy.
    pub policy: VolumePolicy,
    /// Replica placement topology for the volume creation only.
    pub topology: Option<Topology>,
    /// Update of the state in progress.
    #[serde(skip)]
    pub sequencer: OperationSequence,
    /// Id of the last Nexus used by the volume.
    pub last_nexus_id: Option<NexusId>,
    /// Record of the operation in progress
    pub operation: Option<VolumeOperationState>,
    /// Flag indicating whether the volume should be thin provisioned.
    #[serde(default)]
    pub thin: bool,
    /// Last used Target Configuration.
    #[serde(default)]
    pub target_config: Option<TargetConfig>,
    #[serde(default)]
    pub nvmf_parameters: NvmfParameters,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct NvmfParameters {
    pub io_timeout: u32,
    pub ctlr_loss_timeout: u32,
}

impl Default for NvmfParameters {
    fn default() -> Self {
        NvmfParameters {
            io_timeout: 30,
            ctlr_loss_timeout: 1932,
        }
    }
}

/// The volume's Nvmf Configuration.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct TargetConfig {
    /// Uuid of the last target used by the volume.
    target: VolumeTarget,
    /// The nexus target configuration.
    config: NexusNvmfConfig,
}
impl TargetConfig {
    /// Get the uuid of the target.
    pub fn new(target: VolumeTarget, config: NexusNvmfConfig) -> Self {
        Self { target, config }
    }
    /// Get the target.
    pub fn target(&self) -> &VolumeTarget {
        &self.target
    }
    /// Get the uuid of the target.
    pub fn uuid(&self) -> &NexusId {
        &self.target.nexus
    }
    /// Get the target configuration.
    pub fn config(&self) -> &NexusNvmfConfig {
        &self.config
    }
}

impl AsOperationSequencer for VolumeSpec {
    fn as_ref(&self) -> &OperationSequence {
        &self.sequencer
    }

    fn as_mut(&mut self) -> &mut OperationSequence {
        &mut self.sequencer
    }
}

impl VolumeSpec {
    /// Explicitly selected allowed_nodes.
    pub fn allowed_nodes(&self) -> Vec<NodeId> {
        match &self.topology {
            None => vec![],
            Some(t) => t
                .explicit()
                .map(|t| t.allowed_nodes.clone())
                .unwrap_or_default(),
        }
    }
    /// Desired volume replica count if during `SetReplica` operation
    /// or otherwise the current num_replicas.
    pub fn desired_num_replicas(&self) -> u8 {
        match &self.operation {
            Some(operation) => match operation.operation {
                VolumeOperation::SetReplica(count) => count,
                _ => self.num_replicas,
            },
            _ => self.num_replicas,
        }
    }
    /// Get the last target configuration.
    pub fn config(&self) -> &Option<TargetConfig> {
        &self.target_config
    }
    /// Get the health info key which is used to retrieve the volume's replica health information.
    pub fn health_info_id(&self) -> Option<&NexusId> {
        if let Some(id) = &self.last_nexus_id {
            return Some(id);
        }
        self.target_config.as_ref().map(|c| &c.target.nexus)
    }
}

/// Operation State for a Nexus spec resource.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct VolumeOperationState {
    /// Record of the operation.
    pub operation: VolumeOperation,
    /// Result of the operation.
    pub result: Option<bool>,
}

impl From<VolumeOperationState> for models::VolumeSpecOperation {
    fn from(src: VolumeOperationState) -> Self {
        models::VolumeSpecOperation::new_all(src.operation, src.result)
    }
}

impl SpecTransaction<VolumeOperation> for VolumeSpec {
    fn pending_op(&self) -> bool {
        self.operation.is_some()
    }

    fn commit_op(&mut self) {
        if let Some(op) = self.operation.clone() {
            match op.operation {
                VolumeOperation::Destroy => {
                    self.status = SpecStatus::Deleted;
                }
                VolumeOperation::Create => {
                    self.status = SpecStatus::Created(transport::VolumeStatus::Online);
                }
                VolumeOperation::Share(share) => {
                    if let Some(target) = &mut self.target {
                        target.protocol = share.into();
                    }
                }
                VolumeOperation::Unshare => {
                    if let Some(target) = self.target.as_mut() {
                        target.protocol = None
                    }
                }
                VolumeOperation::SetReplica(count) => self.num_replicas = count,
                VolumeOperation::RemoveUnusedReplica(_) => {}
                VolumeOperation::Publish(args) => {
                    let (node, nexus, protocol) = (args.node, args.nexus, args.protocol);
                    let target = VolumeTarget::new(node, nexus, protocol);
                    self.target = Some(target.clone());
                    self.last_nexus_id = None;
                    self.target_config =
                        Some(TargetConfig::new(target, args.config.unwrap_or_default()));
                }
                VolumeOperation::Republish(args) => {
                    let (node, nexus, protocol) = (args.node, args.nexus, args.protocol);
                    let target = VolumeTarget::new(node, nexus, Some(protocol));
                    self.target = Some(target.clone());
                    self.last_nexus_id = None;
                    self.target_config = Some(TargetConfig::new(target, args.config));
                }
                VolumeOperation::Unpublish => {
                    self.target = None;
                }
            }
        }
        self.clear_op();
    }

    fn clear_op(&mut self) {
        self.operation = None;
    }

    fn start_op(&mut self, operation: VolumeOperation) {
        self.operation = Some(VolumeOperationState {
            operation,
            result: None,
        })
    }

    fn set_op_result(&mut self, result: bool) {
        if let Some(op) = &mut self.operation {
            op.result = Some(result);
        }
    }
}

/// Available Volume Operations.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum VolumeOperation {
    Create,
    Destroy,
    Share(VolumeShareProtocol),
    Unshare,
    SetReplica(u8),
    Publish(PublishOperation),
    Republish(RepublishOperation),
    Unpublish,
    RemoveUnusedReplica(ReplicaId),
}

#[test]
fn volume_op_deserializer() {
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct TestSpec {
        op: VolumeOperation,
    }
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct Test<'a> {
        input: &'a str,
        expected: VolumeOperation,
    }
    let tests: Vec<Test> = vec![Test {
        input: r#"{"op":{"Publish":["4ffe7e43-46dd-4912-9d0f-6c9844fa7c6e","4ffe7e43-46dd-4912-9d0f-6c9844fa7c6e",null]}}"#,
        expected: VolumeOperation::Publish(PublishOperation::new(
            "4ffe7e43-46dd-4912-9d0f-6c9844fa7c6e".try_into().unwrap(),
            "4ffe7e43-46dd-4912-9d0f-6c9844fa7c6e".try_into().unwrap(),
            None,
            None,
        )),
    }];

    for test in &tests {
        println!("{}", serde_json::to_string(&test.expected).unwrap());
        let spec: TestSpec = serde_json::from_str(test.input).unwrap();
        assert_eq!(test.expected, spec.op);
    }
}

/// The `PublishOperation` which is easier to manage and update.
#[derive(serde_tuple::Serialize_tuple, serde_tuple::Deserialize_tuple, Debug, Clone, PartialEq)]
pub struct PublishOperation {
    node: NodeId,
    nexus: NexusId,
    protocol: Option<VolumeShareProtocol>,
    #[serde(default)]
    config: Option<NexusNvmfConfig>,
}
impl PublishOperation {
    /// Return new `Self` from the given parameters.
    pub fn new(
        node: NodeId,
        nexus: NexusId,
        protocol: Option<VolumeShareProtocol>,
        config: Option<NexusNvmfConfig>,
    ) -> Self {
        Self {
            node,
            nexus,
            protocol,
            config,
        }
    }
    /// Get the share protocol.
    pub fn protocol(&self) -> Option<VolumeShareProtocol> {
        self.protocol
    }
}

/// Volume Republish Operation parameters.
#[derive(serde_tuple::Serialize_tuple, serde_tuple::Deserialize_tuple, Debug, Clone, PartialEq)]
pub struct RepublishOperation {
    node: NodeId,
    nexus: NexusId,
    protocol: VolumeShareProtocol,
    config: NexusNvmfConfig,
}
impl RepublishOperation {
    /// Return new `Self` from the given parameters.
    pub fn new(
        node: NodeId,
        nexus: NexusId,
        protocol: VolumeShareProtocol,
        config: NexusNvmfConfig,
    ) -> Self {
        Self {
            node,
            nexus,
            protocol,
            config,
        }
    }
    /// Get the share protocol.
    pub fn protocol(&self) -> VolumeShareProtocol {
        self.protocol
    }
}

impl From<VolumeOperation> for models::volume_spec_operation::Operation {
    fn from(src: VolumeOperation) -> Self {
        match src {
            VolumeOperation::Create => models::volume_spec_operation::Operation::Create,
            VolumeOperation::Destroy => models::volume_spec_operation::Operation::Destroy,
            VolumeOperation::Share(_) => models::volume_spec_operation::Operation::Share,
            VolumeOperation::Unshare => models::volume_spec_operation::Operation::Unshare,
            VolumeOperation::SetReplica(_) => models::volume_spec_operation::Operation::SetReplica,
            VolumeOperation::Publish(_) => models::volume_spec_operation::Operation::Publish,
            VolumeOperation::Republish(_) => models::volume_spec_operation::Operation::Republish,
            VolumeOperation::Unpublish => models::volume_spec_operation::Operation::Unpublish,
            VolumeOperation::RemoveUnusedReplica(_) => {
                models::volume_spec_operation::Operation::RemoveUnusedReplica
            }
        }
    }
}

/// Key used by the store to uniquely identify a VolumeSpec structure.
pub struct VolumeSpecKey(VolumeId);

impl From<&VolumeId> for VolumeSpecKey {
    fn from(id: &VolumeId) -> Self {
        Self(id.clone())
    }
}

impl ObjectKey for VolumeSpecKey {
    fn key_type(&self) -> StorableObjectType {
        StorableObjectType::VolumeSpec
    }

    fn key_uuid(&self) -> String {
        self.0.to_string()
    }
}

impl StorableObject for VolumeSpec {
    type Key = VolumeSpecKey;

    fn key(&self) -> Self::Key {
        VolumeSpecKey(self.uuid.clone())
    }
}

/// State of the Volume Spec.
pub type VolumeSpecStatus = SpecStatus<transport::VolumeStatus>;

impl From<&CreateVolume> for VolumeSpec {
    fn from(request: &CreateVolume) -> Self {
        Self {
            uuid: request.uuid.clone(),
            size: request.size,
            labels: request.labels.clone(),
            num_replicas: request.replicas as u8,
            status: VolumeSpecStatus::Creating,
            target: None,
            policy: request.policy.clone(),
            topology: request.topology.clone(),
            sequencer: OperationSequence::new(request.uuid.clone()),
            last_nexus_id: None,
            operation: None,
            thin: request.thin,
            target_config: None,
            nvmf_parameters: request.nvmf_parameters.clone(),
        }
    }
}
impl PartialEq<CreateVolume> for VolumeSpec {
    fn eq(&self, other: &CreateVolume) -> bool {
        let mut other = VolumeSpec::from(other);
        other.status = self.status.clone();
        other.sequencer = self.sequencer.clone();
        &other == self
    }
}
impl From<&VolumeSpec> for transport::VolumeState {
    fn from(spec: &VolumeSpec) -> Self {
        Self {
            uuid: spec.uuid.clone(),
            size: spec.size,
            status: transport::VolumeStatus::Unknown,
            target: None,
            replica_topology: HashMap::new(),
        }
    }
}
impl PartialEq<transport::VolumeState> for VolumeSpec {
    fn eq(&self, other: &transport::VolumeState) -> bool {
        match &self.target {
            None => other.target_node().flatten().is_none(),
            Some(target) => {
                target.protocol == other.target_protocol()
                    && Some(&target.node) == other.target_node().flatten().as_ref()
                    && other.status == VolumeStatus::Online
            }
        }
    }
}

impl From<VolumeSpec> for models::VolumeSpec {
    fn from(src: VolumeSpec) -> Self {
        Self::new_all(
            src.labels,
            src.num_replicas,
            src.operation.into_opt(),
            src.size,
            src.status,
            src.target.into_opt(),
            src.uuid,
            src.topology.into_opt(),
            src.policy,
            src.thin,
        )
    }
}
