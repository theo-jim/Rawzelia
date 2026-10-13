use std::collections::BTreeMap;

use rawzelia_domain::{Adjustment, EditDocument, Fingerprint};
use rawzelia_storage::{Reconciliation, XmpSnapshot, reconcile_sources};

fn document(pipeline_version: &str, exposure: f32) -> EditDocument {
    EditDocument {
        schema_version: 1,
        source_fingerprint: Fingerprint {
            size: 4096,
            sha256: [11; 32],
        },
        pipeline_version: pipeline_version.to_owned(),
        adjustments: vec![Adjustment::Exposure { value: exposure }],
        community: Default::default(),
        lightroom: Vec::new(),
        lightroom_unknown: Vec::new(),
        pro: Default::default(),
    }
}

#[test]
fn divergent_sidecar_requires_explicit_resolution() {
    let catalog = document("0.1.0", 0.25);
    let sidecar = document("0.1.1", 0.75);
    let xmp = XmpSnapshot {
        fields: BTreeMap::from([("crs:Exposure2012".to_owned(), "0.25".to_owned())]),
    };

    let result = reconcile_sources(&catalog, &sidecar, &xmp)
        .expect("divergence must be classified without mutation");
    let Reconciliation::Conflict { fields } = result else {
        panic!("divergent sidecar must require explicit resolution");
    };
    assert_eq!(
        fields,
        vec!["adjustments".to_owned(), "pipeline_version".to_owned()]
    );
    assert_eq!(catalog, document("0.1.0", 0.25));
    assert_eq!(sidecar, document("0.1.1", 0.75));

    assert_eq!(
        reconcile_sources(
            &catalog,
            &catalog,
            &XmpSnapshot {
                fields: BTreeMap::new(),
            },
        )
        .expect("identical sources must reconcile"),
        Reconciliation::CatalogAuthoritative
    );
}
