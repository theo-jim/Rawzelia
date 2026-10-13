use serde::Serialize;

pub const CONTRACT_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum Capability {
    Community,
    Pro,
    LightroomReadOnly,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CapabilitySet {
    pub contract_version: u16,
    pub capabilities: Vec<Capability>,
}

pub fn community_capabilities() -> CapabilitySet {
    CapabilitySet {
        contract_version: CONTRACT_VERSION,
        capabilities: vec![Capability::Community, Capability::LightroomReadOnly],
    }
}

pub fn pro_capabilities() -> CapabilitySet {
    CapabilitySet {
        contract_version: CONTRACT_VERSION,
        capabilities: vec![
            Capability::Community,
            Capability::Pro,
            Capability::LightroomReadOnly,
        ],
    }
}
