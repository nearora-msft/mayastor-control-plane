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

/// NexusSpec : User specification of a nexus.

/// User specification of a nexus.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NexusSpec {
    /// List of children.
    #[serde(rename = "children")]
    pub children: Vec<String>,
    /// Managed by our control plane
    #[serde(rename = "managed")]
    pub managed: bool,
    /// Node where the nexus should live.
    #[serde(rename = "node")]
    pub node: String,
    #[serde(rename = "operation", skip_serializing_if = "Option::is_none")]
    pub operation: Option<crate::models::NexusSpecOperation>,
    /// Volume which owns this nexus, if any
    #[serde(rename = "owner", skip_serializing_if = "Option::is_none")]
    pub owner: Option<uuid::Uuid>,
    #[serde(rename = "share")]
    pub share: crate::models::Protocol,
    /// Size of the nexus.
    #[serde(rename = "size")]
    pub size: u64,
    #[serde(rename = "state")]
    pub state: crate::models::SpecState,
    /// Nexus Id
    #[serde(rename = "uuid")]
    pub uuid: uuid::Uuid,
}

impl NexusSpec {
    /// NexusSpec using only the required fields
    pub fn new(
        children: impl IntoVec<String>,
        managed: impl Into<bool>,
        node: impl Into<String>,
        share: impl Into<crate::models::Protocol>,
        size: impl Into<u64>,
        state: impl Into<crate::models::SpecState>,
        uuid: impl Into<uuid::Uuid>,
    ) -> NexusSpec {
        NexusSpec {
            children: children.into_vec(),
            managed: managed.into(),
            node: node.into(),
            operation: None,
            owner: None,
            share: share.into(),
            size: size.into(),
            state: state.into(),
            uuid: uuid.into(),
        }
    }
    /// NexusSpec using all fields
    pub fn new_all(
        children: impl IntoVec<String>,
        managed: impl Into<bool>,
        node: impl Into<String>,
        operation: impl Into<Option<crate::models::NexusSpecOperation>>,
        owner: impl Into<Option<uuid::Uuid>>,
        share: impl Into<crate::models::Protocol>,
        size: impl Into<u64>,
        state: impl Into<crate::models::SpecState>,
        uuid: impl Into<uuid::Uuid>,
    ) -> NexusSpec {
        NexusSpec {
            children: children.into_vec(),
            managed: managed.into(),
            node: node.into(),
            operation: operation.into(),
            owner: owner.into(),
            share: share.into(),
            size: size.into(),
            state: state.into(),
            uuid: uuid.into(),
        }
    }
}
