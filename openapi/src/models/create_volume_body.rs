#![allow(
    clippy::too_many_arguments,
    clippy::new_without_default,
    non_camel_case_types,
    unused_imports
)]
/*
 * Mayastor RESTful API
 *
 * The version of the OpenAPI document: v0
 *
 * Generated by: https://github.com/openebs/openapi-generator
 */

use crate::apis::IntoVec;

/// CreateVolumeBody : Create Volume Body JSON

/// Create Volume Body JSON
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateVolumeBody {
    /// Volume Healing policy used to determine if and how to replace a replica
    #[serde(rename = "policy")]
    pub policy: crate::models::VolumeHealPolicy,
    /// number of storage replicas
    #[serde(rename = "replicas")]
    pub replicas: u8,
    /// size of the volume in bytes
    #[serde(rename = "size")]
    pub size: u64,
    /// Volume topology used to determine how to place/distribute the data.  Should either be labelled or explicit, not both.  If neither is used then the control plane will select from all available resources.
    #[serde(rename = "topology")]
    pub topology: crate::models::Topology,
}

impl CreateVolumeBody {
    /// CreateVolumeBody using only the required fields
    pub fn new(
        policy: impl Into<crate::models::VolumeHealPolicy>,
        replicas: impl Into<u8>,
        size: impl Into<u64>,
        topology: impl Into<crate::models::Topology>,
    ) -> CreateVolumeBody {
        CreateVolumeBody {
            policy: policy.into(),
            replicas: replicas.into(),
            size: size.into(),
            topology: topology.into(),
        }
    }
    /// CreateVolumeBody using all fields
    pub fn new_all(
        policy: impl Into<crate::models::VolumeHealPolicy>,
        replicas: impl Into<u8>,
        size: impl Into<u64>,
        topology: impl Into<crate::models::Topology>,
    ) -> CreateVolumeBody {
        CreateVolumeBody {
            policy: policy.into(),
            replicas: replicas.into(),
            size: size.into(),
            topology: topology.into(),
        }
    }
}
