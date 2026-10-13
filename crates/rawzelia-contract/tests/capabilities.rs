use rawzelia_contract::{Capability, community_capabilities};

#[test]
fn community_capabilities_are_offline_and_versioned() {
    let capabilities = community_capabilities();
    let serialized = serde_json::to_value(&capabilities).expect("capabilities serialize");

    assert_eq!(serialized["contractVersion"], 1);
    assert!(serialized.get("contract_version").is_none());
    assert!(capabilities.capabilities.contains(&Capability::Community));
    assert!(!capabilities.capabilities.contains(&Capability::Pro));
    assert!(
        capabilities
            .capabilities
            .contains(&Capability::LightroomReadOnly)
    );
}
