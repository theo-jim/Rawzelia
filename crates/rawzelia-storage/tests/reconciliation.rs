use std::collections::BTreeMap;

use rawzelia_domain::{Adjustment, EditDocument, Fingerprint};
use rawzelia_storage::{
    Reconciliation, XmpSnapshot, reconcile_sources, reconcile_sources_with_optional_xmp,
};

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
}

#[test]
fn present_xmp_with_missing_fields_requires_explicit_resolution() {
    let mut catalog = document("0.1.0", 0.25);
    catalog
        .adjustments
        .push(Adjustment::Contrast { value: 12.0 });
    let sidecar = catalog.clone();
    let deleted_contrast = XmpSnapshot {
        fields: BTreeMap::from([("crs:Exposure2012".to_owned(), "0.25".to_owned())]),
    };

    assert_eq!(
        reconcile_sources(&catalog, &sidecar, &deleted_contrast)
            .expect("a deleted XMP field must be classified"),
        Reconciliation::Conflict {
            fields: vec!["xmp.crs:Contrast2012".to_owned()],
        }
    );

    let present_empty = XmpSnapshot {
        fields: BTreeMap::new(),
    };
    assert_eq!(
        reconcile_sources(&catalog, &sidecar, &present_empty)
            .expect("a present empty XMP must be classified"),
        Reconciliation::Conflict {
            fields: vec![
                "xmp.crs:Contrast2012".to_owned(),
                "xmp.crs:Exposure2012".to_owned(),
            ],
        }
    );
}

#[test]
fn absent_xmp_source_is_explicit() {
    let catalog = document("0.1.0", 0.25);
    let sidecar = catalog.clone();

    assert_eq!(
        reconcile_sources_with_optional_xmp(&catalog, &sidecar, None)
            .expect("an absent XMP must be represented explicitly"),
        Reconciliation::CatalogAuthoritative
    );

    assert_eq!(
        reconcile_sources_with_optional_xmp(
            &catalog,
            &sidecar,
            Some(&XmpSnapshot {
                fields: BTreeMap::new(),
            }),
        )
        .expect("a present empty XMP must remain distinguishable"),
        Reconciliation::Conflict {
            fields: vec!["xmp.crs:Exposure2012".to_owned()],
        }
    );
}
