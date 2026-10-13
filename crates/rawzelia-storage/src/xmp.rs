use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use rawzelia_domain::{Adjustment, CoreValue, EditDocument, ImportStatus, ImportedField};

use crate::StorageError;
use crate::sidecar::write_bytes_atomic;

const RAWZELIA_XMP_PREFIX: &str =
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- rawzelia:xmp-mirror:v1 -->\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmpSnapshot {
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reconciliation {
    CatalogAuthoritative,
    SidecarAuthoritative,
    XmpAuthoritative,
    Conflict { fields: Vec<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XmpWriteReport {
    pub mirrored_fields: u32,
    pub skipped_fields: u32,
}

pub fn write_xmp_mirror(
    path: &Path,
    document: &EditDocument,
) -> Result<XmpWriteReport, StorageError> {
    let managed_target = path
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.ends_with(".rawzelia.xmp"));
    if !managed_target {
        return Err(unmanaged_xmp_target());
    }
    match fs::read(path) {
        Ok(bytes) if bytes.starts_with(RAWZELIA_XMP_PREFIX.as_bytes()) => {}
        Ok(_) => return Err(unmanaged_xmp_target()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(StorageError::from_io(error)),
    }

    let (properties, skipped_fields) = mirror_properties(document);
    let mut xml = String::from(RAWZELIA_XMP_PREFIX);
    xml.push_str(
        "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n\
         <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\
         <rdf:Description xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\" \
         xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
         xmlns:tiff=\"http://ns.adobe.com/tiff/1.0/\" \
         xmlns:crs=\"http://ns.adobe.com/camera-raw-settings/1.0/\">\n",
    );
    for (name, value) in &properties {
        xml.push('<');
        xml.push_str(name);
        xml.push('>');
        xml.push_str(&escape_xml(value));
        xml.push_str("</");
        xml.push_str(name);
        xml.push_str(">\n");
    }
    xml.push_str("</rdf:Description>\n</rdf:RDF>\n</x:xmpmeta>\n");
    write_bytes_atomic(path, xml.as_bytes())?;

    Ok(XmpWriteReport {
        mirrored_fields: u32::try_from(properties.len()).map_err(|_| StorageError::Io {
            code: "xmp_field_count_out_of_range".to_owned(),
            message_key: "storage.xmp.too_many_fields".to_owned(),
        })?,
        skipped_fields,
    })
}

fn unmanaged_xmp_target() -> StorageError {
    StorageError::Io {
        code: "unmanaged_xmp_target".to_owned(),
        message_key: "storage.xmp.unmanaged_target".to_owned(),
    }
}

pub fn reconcile_sources(
    catalog: &EditDocument,
    sidecar: &EditDocument,
    xmp: &XmpSnapshot,
) -> Result<Reconciliation, StorageError> {
    reconcile_sources_with_optional_xmp(catalog, sidecar, Some(xmp))
}

pub fn reconcile_sources_with_optional_xmp(
    catalog: &EditDocument,
    sidecar: &EditDocument,
    xmp: Option<&XmpSnapshot>,
) -> Result<Reconciliation, StorageError> {
    let document_conflicts = document_conflicts(catalog, sidecar);
    if !document_conflicts.is_empty() {
        return Ok(Reconciliation::Conflict {
            fields: document_conflicts,
        });
    }

    let Some(xmp) = xmp else {
        return Ok(Reconciliation::CatalogAuthoritative);
    };

    let (catalog_xmp, _) = mirror_properties(catalog);
    let mut conflicts = BTreeSet::new();
    for field in catalog_xmp.keys().chain(xmp.fields.keys()) {
        if catalog_xmp.get(field) != xmp.fields.get(field) {
            conflicts.insert(format!("xmp.{field}"));
        }
    }
    if conflicts.is_empty() {
        Ok(Reconciliation::CatalogAuthoritative)
    } else {
        Ok(Reconciliation::Conflict {
            fields: conflicts.into_iter().collect(),
        })
    }
}

fn mirror_properties(document: &EditDocument) -> (BTreeMap<String, String>, u32) {
    let mut properties = BTreeMap::new();
    let mut skipped = 0_u32;

    for (field, imported) in &document.community {
        if !matches!(
            imported.status,
            ImportStatus::Exact | ImportStatus::Approximated
        ) || !insert_community_property(&mut properties, field, imported)
        {
            skipped = skipped.saturating_add(1);
        }
    }
    for adjustment in &document.adjustments {
        insert_adjustment_properties(&mut properties, adjustment);
    }
    skipped = skipped
        .saturating_add(u32::try_from(document.lightroom.len()).unwrap_or(u32::MAX))
        .saturating_add(u32::try_from(document.lightroom_unknown.len()).unwrap_or(u32::MAX))
        .saturating_add(u32::try_from(document.pro.len()).unwrap_or(u32::MAX));

    (properties, skipped)
}

fn insert_community_property(
    properties: &mut BTreeMap<String, String>,
    field: &str,
    imported: &ImportedField<CoreValue>,
) -> bool {
    let Some(value) = imported.value.as_ref() else {
        return false;
    };
    let (xmp_field, rendered) = match (field, value) {
        ("community.rating", CoreValue::Integer(value)) => ("xmp:Rating", value.to_string()),
        ("community.flag", CoreValue::Text(value)) => ("crs:Pick", value.clone()),
        ("community.label", CoreValue::Text(value)) => ("xmp:Label", value.clone()),
        ("community.title", CoreValue::Text(value)) => ("dc:title", value.clone()),
        ("community.description", CoreValue::Text(value)) => ("dc:description", value.clone()),
        ("community.rights", CoreValue::Text(value)) => ("dc:rights", value.clone()),
        ("community.orientation", CoreValue::Integer(value)) => {
            ("tiff:Orientation", value.to_string())
        }
        ("community.keywords", CoreValue::Text(value)) => ("dc:subject", value.clone()),
        _ => return false,
    };
    properties.insert(xmp_field.to_owned(), rendered);
    true
}

fn insert_adjustment_properties(
    properties: &mut BTreeMap<String, String>,
    adjustment: &Adjustment,
) {
    match adjustment {
        Adjustment::Exposure { value } => {
            properties.insert("crs:Exposure2012".to_owned(), value.to_string());
        }
        Adjustment::Contrast { value } => {
            properties.insert("crs:Contrast2012".to_owned(), value.to_string());
        }
        Adjustment::Highlights { value } => {
            properties.insert("crs:Highlights2012".to_owned(), value.to_string());
        }
        Adjustment::Shadows { value } => {
            properties.insert("crs:Shadows2012".to_owned(), value.to_string());
        }
        Adjustment::WhiteBalance { temperature, tint } => {
            properties.insert("crs:Temperature".to_owned(), temperature.to_string());
            properties.insert("crs:Tint".to_owned(), tint.to_string());
        }
        Adjustment::Crop {
            x,
            y,
            width,
            height,
        } => {
            properties.insert("crs:CropLeft".to_owned(), x.to_string());
            properties.insert("crs:CropTop".to_owned(), y.to_string());
            properties.insert("crs:CropWidth".to_owned(), width.to_string());
            properties.insert("crs:CropHeight".to_owned(), height.to_string());
        }
        Adjustment::Rotation { degrees } => {
            properties.insert("crs:Rotation".to_owned(), degrees.to_string());
        }
        Adjustment::Straighten { degrees } => {
            properties.insert("crs:StraightenAngle".to_owned(), degrees.to_string());
        }
        Adjustment::LensCorrection { profile, amount } => {
            properties.insert("crs:LensProfile".to_owned(), profile.clone());
            properties.insert("crs:LensProfileAmount".to_owned(), amount.to_string());
        }
    }
}

fn document_conflicts(catalog: &EditDocument, sidecar: &EditDocument) -> Vec<String> {
    let mut fields = Vec::new();
    if catalog.adjustments != sidecar.adjustments {
        fields.push("adjustments".to_owned());
    }
    if catalog.community != sidecar.community {
        fields.push("community".to_owned());
    }
    if catalog.lightroom != sidecar.lightroom {
        fields.push("lightroom".to_owned());
    }
    if catalog.lightroom_unknown != sidecar.lightroom_unknown {
        fields.push("lightroom_unknown".to_owned());
    }
    if catalog.pipeline_version != sidecar.pipeline_version {
        fields.push("pipeline_version".to_owned());
    }
    if catalog.pro != sidecar.pro {
        fields.push("pro".to_owned());
    }
    if catalog.schema_version != sidecar.schema_version {
        fields.push("schema_version".to_owned());
    }
    if catalog.source_fingerprint != sidecar.source_fingerprint {
        fields.push("source_fingerprint".to_owned());
    }
    fields.sort();
    fields
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
