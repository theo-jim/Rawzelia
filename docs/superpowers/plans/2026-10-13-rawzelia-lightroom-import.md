# Lightroom Classic Import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Importer le catalogue Lightroom Classic le plus récent présent dans le corpus utilisateur, en lecture seule, avec dry-run explicite, provenance par champ, conservation des données non mappées et import transactionnel réversible vers un catalogue Rawzelia séparé.

**Architecture:** Le garde source copie le catalogue choisi dans un workspace immuable après avoir refusé toute source active ou verrouillée. Un adaptateur isolé lit cette copie, normalise les champs connus dans des sous-types Rust et laisse les données inconnues sous lightroom.*; l’application orchestre un rapport puis une transaction SQLite vérifiable et annulable. L’interface Tauri ne fait que transporter les commandes métier typées.

**Tech Stack:** Rust stable, workspace Cargo, SQLite/WAL via rusqlite, sérialisation serde, hachage SHA-256, parsing XMP/XML et .lrtemplate, Tauri 2 pour l’inspecteur, tests Rust et fixtures golden externes injectées par RAWZELIA_FIXTURES_DIR.

**Spec:** docs/superpowers/specs/2026-10-12-rawzelia-v0.1-design.md

## Global Constraints

- Le périmètre de ce plan est l’import Lightroom des semaines 9 à 16; les adaptateurs de versions antérieures restent hors annonce jusqu’à leurs fixtures et tests dédiés.
- Le premier adaptateur cible uniquement le schéma Lightroom le plus récent présent dans le corpus utilisateur; aucune mise à niveau Adobe préalable n’est requise.
- Toutes les lectures du .lrcat, de .lrdata, des XMP, des presets et des originaux sont read-only; aucune tâche n’écrit dans ces sources.
- Le catalogue source est refusé s’il est actif ou verrouillé, puis copié vers un workspace avant inspection ou extraction.
- Le dry-run précède toute transaction et le commit n’est autorisé qu’après vérification des comptages, IDs source et fingerprints.
- Le core produit et exporte le module canonique `rawzelia-contract/src/lightroom_import.rs`. Ce plan le consomme sans le recréer; les types exacts LightroomSchemaId, ImportStatus, FieldProvenance, ImportedField et UnknownField sont uniques.
- `rawzelia-contract/src/commands.rs`, créé par le core, reste l’unique propriétaire de `AppCommand`, `InspectLightroomCommand`, `ExecuteLightroomCommand`, `RollbackLightroomCommand` et `ReferenceJpegSelection`; `rawzelia-app/src/lightroom_import.rs` les consomme et les orchestre sans redéfinir ces types.
- WorkspaceCopy, ImportError, LightroomAdapter, ImportPlan et LightroomBatchStore sont également uniques dans leurs owners ci-dessous; aucune variante homonyme n’est créée.
- `rawzelia-io::IngestPlan` est réservé au flux carte SD. Le plan core doit appliquer ce renommage avant intégration. `rawzelia-app::lightroom_import::ImportPlan` est réservé à Lightroom et ne consomme jamais IngestPlan.
- LightroomBatchStore réutilise `rawzelia-storage::StorageError`, avec `From<StorageError> for ImportError`; aucune erreur de stockage concurrente n’est définie ici.
- Les records connus sont typés par sous-types; serde_json::Value est réservé au payload brut de UnknownField pour les champs inconnus lightroom.*.
- Les statuts sérialisés sont exactement exact, approximated, preserved_only, unsupported et ignored.
- Les réglages Develop conservés ou mappés ne promettent jamais un rendu Adobe identique.
- Le JPEG de référence Lightroom est associé uniquement après sélection explicite et consentement de l’utilisateur; aucune association automatique ambiguë n’est autorisée.
- Le workspace Rust doit être bootstrapé par le plan core avant l’exécution de ce plan; ce plan ne redéfinit pas les crates déjà possédées par le core.
- Toute étape qui produit un commit dans ce plan est une instruction pour l’exécutant; la présente demande de révision ne crée ni commit ni modification d’un autre fichier.

## Review Focus

- Catalogue .lrcat actif ou verrouillé : appeler validate_source_guard avant toute copie et mapper le résultat — Task 2 / rejects_lrcat_lock_before_copy, same_lrcat_lock_maps_to_same_source_locked_code.
- Manifest golden ou corpus absent : échouer au préflight avec chemin attendu, sans embarquer de binaire Adobe — Task 3 / manifest_preflight_requires_external_catalog.
- Volume source manquant ou fingerprint divergent : produire un rapport actionnable et empêcher le commit — Task 4 / dry_run_reports_schema_counts_ids_fingerprints_and_statuses_without_mutation.
- .lrdata absent, preset invalide ou donnée de plugin inconnue : continuer les autres champs avec statut et provenance — Task 6 / missing_lrdata_is_non_fatal, invalid_preset_is_preserved_and_classified, unknown_plugin_data_is_preserved.
- JPEG de référence ambigu ou rollback après association : exiger un choix explicite, vérifier le fingerprint et supprimer seulement le lot — Task 6 / ambiguous_reference_jpeg_is_rejected, Task 7 / desktop_reference_jpeg_is_removed_after_rollback, Task 8 / reference_jpeg_is_rolled_back_with_batch.

## Contrats et ownership

Le plan core peut fournir une façade provisoire pour démarrer le workspace, mais elle doit s’adapter à ces contrats au moment de l’intégration. Les noms ci-dessous sont la ledger normative pour les plans import et hardening; RecoveryStatus, Provenance, ValidatedLightroomSource et un second LightroomAdapter ne doivent pas devenir des contrats concurrents.

| Élément canonique | Producteur et fichier owner | Consommateurs | Ownership/mutation |
|---|---|---|---|
| LightroomSchemaId, ImportStatus, FieldProvenance, ImportedField<T>, UnknownField | crates/rawzelia-contract/src/lightroom_import.rs, produit par core | domain, import, storage, desktop, hardening | types immuables; ce plan les consomme sans les redéfinir |
| WorkspaceCopy | crates/rawzelia-lightroom-import/src/workspace.rs | adapter, dry-run, artefacts | le module workspace possède la copie; les lecteurs n’ont qu’un emprunt read-only |
| ImportError | crates/rawzelia-lightroom-import/src/errors.rs | garde, adapter, app, storage, desktop | mapping explicite des gardes et erreurs de stockage; aucune écriture implicite |
| LightroomAdapter | crates/rawzelia-lightroom-import/src/adapter.rs | sélection latest, app, tests golden | l’adaptateur possède la lecture de sa version; le sink possède les records émis |
| ImportPlan | crates/rawzelia-app/src/lightroom_import.rs | commit, desktop, tests | plan Lightroom immuable après dry-run; l’app possède sa validation |
| IngestPlan | crates/rawzelia-io/src/ingest.rs, produit par core | flux carte SD uniquement | contrat distinct; jamais accepté par l’import Lightroom |
| LightroomBatchStore | crates/rawzelia-storage/src/lightroom_batch.rs | app, tests transactionnels | le store possède la transaction du catalogue cible; rollback limité au batch_id |
| StorageError | crates/rawzelia-storage/src/catalog.rs, produit par core | LightroomBatchStore, ImportError | enum partagé; converti par From<StorageError> |
| AppCommand, InspectLightroomCommand, ExecuteLightroomCommand, RollbackLightroomCommand, ReferenceJpegSelection | crates/rawzelia-contract/src/commands.rs, produits par le core | rawzelia-app, desktop, bridge Tauri, transaction | propriétaire unique du transport et de la sélection; les consommateurs conservent le même type et le même fingerprint après consentement |
| FixtureManifest | crates/rawzelia-lightroom-import/src/fixture_preflight.rs | latest, legacy, hardening | modèle extensible; hardening modifie le manifest et l’étend sans le redéfinir |

Les types auxiliaires sont également décidés ici et réutilisés sans redéfinition : `pub struct LightroomSourceIdentity { pub catalog_path: std::path::PathBuf, pub schema: LightroomSchemaId, pub fingerprint: Fingerprint }`; `pub struct ImportReportId(pub String)`; `pub struct ImportBatchId(pub String)`; `pub struct ImportCounts { pub assets: u64, pub folders: u64, pub collections: u64, pub collection_sets: u64, pub smart_collections: u64, pub stacks: u64, pub virtual_copies: u64 }`; `pub struct ImportIssue { pub source_id: Option<String>, pub field_path: Option<String>, pub status: ImportStatus, pub message_key: String }`; `pub struct ImportReport { pub report_id: ImportReportId, pub source: LightroomSourceIdentity, pub counts: ImportCounts, pub statuses: std::collections::BTreeMap<ImportStatus, u64>, pub issues: Vec<ImportIssue> }`; `pub struct DesktopError { pub code: String, pub message_key: String }`; `pub struct TestError { pub message: String }`. Le core possède `Fingerprint` et `StorageError`; aucun alias local ne les remplace.

### Task 1: Records normalisés et sélection JPEG typée, semaine 9

**Files:**
- Create: crates/rawzelia-lightroom-import/src/records.rs
- Create: crates/rawzelia-lightroom-import/src/reference.rs
- Modify: crates/rawzelia-lightroom-import/src/lib.rs
- Test: crates/rawzelia-lightroom-import/tests/import_contract.rs

**Interfaces:**
- Consumes: `rawzelia_contract::lightroom_import::{LightroomSchemaId, ImportStatus, FieldProvenance, ImportedField, UnknownField}`, `rawzelia_contract::commands::{ReferenceChoice, ReferenceJpegSelection}` et `rawzelia_contract::types::Fingerprint`; aucun accès SQLite ou fichier source.
- Produces: `ImportedRecord` et ses sous-types Rust sérialisables; aucun de ces types ne redéfinit les cinq types canoniques.
- Produces: pub struct ImportedRecord { pub asset: ImportedAsset, pub metadata: ImportedMetadata, pub organization: ImportedOrganization, pub relationships: Vec<ImportedRelationship>, pub previews: ImportedPreviews, pub develop: ImportedDevelop, pub unknown_fields: Vec<UnknownField> }.
- Produces: pub struct ImportedAsset { pub source_id: String, pub original_path: ImportedField<std::path::PathBuf>, pub volume: ImportedField<String>, pub folder: ImportedField<String>, pub capture_date: ImportedField<String> }.
- Produces: pub struct ImportedMetadata { pub exif: Vec<ImportedField<ExifValue>>, pub iptc: Vec<ImportedField<IptcValue>>, pub gps: Vec<ImportedField<GpsValue>>, pub captions: Vec<ImportedField<String>>, pub rights: Vec<ImportedField<String>>, pub notes: Vec<ImportedField<String>> }.
- Produces: pub struct ImportedOrganization { pub rating: Option<ImportedField<u8>>, pub flag: Option<ImportedField<FlagValue>>, pub label: Option<ImportedField<ColorLabel>>, pub keywords: Vec<ImportedField<String>> }.
- Produces: pub struct ImportedRelationship { pub kind: RelationshipKind, pub source_id: String, pub target_id: String, pub status: ImportStatus, pub provenance: FieldProvenance }.
- Produces: pub struct ImportedPreviews { pub embedded: Option<ImportedArtifact>, pub standard: Option<ImportedArtifact>, pub smart: Option<ImportedArtifact> }.
- Produces: pub struct ImportedDevelop { pub history: Vec<ImportedField<DevelopStep>>, pub settings: Vec<ImportedField<DevelopValue>>, pub preserved: Vec<ImportedField<DevelopValue>> }.
- Produces: `pub struct ImportedArtifact { pub kind: ArtifactKind, pub path: std::path::PathBuf, pub fingerprint: Fingerprint, pub status: ImportStatus, pub provenance: FieldProvenance }`.
- Produces: `pub fn imported_field<T>(value: Option<T>, status: ImportStatus, provenance: FieldProvenance) -> ImportedField<T>`.

Les enums ExifValue, IptcValue, GpsValue, FlagValue, ColorLabel, RelationshipKind, ArtifactKind, DevelopStep et DevelopValue sont des sous-types Rust sérialisables, chacun avec les variantes adaptées au domaine; aucun n’emploie serde_json::Value. `SourceRelationship` complète la représentation lue. Les champs connus de métadonnées, d’organisation, de relations, de previews et de Develop passent par ImportedField<T>. UnknownField.raw_value est l’unique exception JSON, toujours sous un chemin lightroom.*. `ImportError` est produit par Task 2.

- [ ] **Step 1: Écrire le test rouge**

  Tests canonical_core_types_are_reused_without_redefinition, known_records_are_typed_subrecords et unknown_fields_are_the_only_json_values. Vérifier les cinq statuts importés depuis le module core, la provenance par champ et la composition de ImportedRecord sans carte JSON pour les champs connus.

- [ ] **Step 2: Exécuter le test pour vérifier l’échec**

  Run: cargo test -p rawzelia-lightroom-import --test import_contract -- canonical_core_types_are_reused_without_redefinition --exact

  Expected: FAIL avec une erreur de compilation indiquant que ImportedRecord ou l’import du type canonique `ReferenceJpegSelection` depuis `rawzelia-contract::commands` n’est pas encore câblé.

- [ ] **Step 3: Implémentation minimale des records dans les fichiers owners**

  Importer les cinq types depuis `rawzelia_contract::lightroom_import` et `ReferenceChoice`/`ReferenceJpegSelection` depuis `rawzelia_contract::commands`. Définir uniquement les records et sous-types de l’importeur avec serde et des conversions explicites. `reference.rs` ne définit aucun type de sélection; ne créer aucun alias concurrent nommé RecoveryStatus ou Provenance.

- [ ] **Step 4: Exécuter le test pour vérifier la réussite**

  Run: cargo test -p rawzelia-lightroom-import --test import_contract && rg -n "serde_json::Value" crates/rawzelia-lightroom-import/src

  Expected: PASS; l’usage autorisé de serde_json::Value est limité à UnknownField.raw_value du contrat core et aux inconnus transportés par l’importeur.

- [ ] **Step 5: Commit**

  ~~~bash
  git add crates/rawzelia-lightroom-import
  git commit -m "feat: define normalized Lightroom import records"
  ~~~

### Task 2: Garde source et copie de workspace read-only, semaines 9-10

**Files:**
- Create: crates/rawzelia-lightroom-import/src/source.rs
- Create: crates/rawzelia-lightroom-import/src/workspace.rs
- Create: crates/rawzelia-lightroom-import/src/errors.rs
- Modify: crates/rawzelia-lightroom-import/src/lib.rs
- Test: crates/rawzelia-lightroom-import/tests/source_safety.rs

**Interfaces:**
- Consumes: LightroomSchemaId, Fingerprint, LightroomSourceIdentity et ImportError de l’importeur; `rawzelia_lightroom_import::source_guard::validate_source_guard` et `SourceGuardError` sont produits par core.
- Produces: pub enum LockState { Unlocked, Active, Locked, Corrupt }.
- Produces: pub struct SourceInspection { pub identity: LightroomSourceIdentity, pub lock_state: LockState, pub catalog_path: std::path::PathBuf, pub associated_root: Option<std::path::PathBuf> }.
- Produces: pub struct WorkspaceCopy { pub root: std::path::PathBuf, pub catalog_path: std::path::PathBuf, pub associated_root: Option<std::path::PathBuf>, pub source: LightroomSourceIdentity, pub workspace_fingerprint: Fingerprint }.
- Produces: pub fn inspect_source(path: &std::path::Path) -> Result<SourceInspection, ImportError>.
- Produces: pub fn prepare_workspace(inspection: &SourceInspection, workspace_root: &std::path::Path) -> Result<WorkspaceCopy, ImportError>.
- Produces: pub fn verify_source_unchanged(inspection: &SourceInspection) -> Result<(), ImportError>.
- Produces: pub enum ImportErrorCode { SourceMissing, SourceLocked, SourceActive, SourceCorrupt, FixtureMissing, SchemaUnsupported, FingerprintMismatch, AmbiguousReference, InvalidPreset, StorageFailure }.
- Produces: pub struct ImportError { pub code: ImportErrorCode, pub message_key: String, pub source_id: Option<String>, pub field_path: Option<String> }.
- Produces: pub fn map_source_guard_error(error: SourceGuardError) -> ImportError; Missing -> SourceMissing, Active -> SourceActive, Locked -> SourceLocked, Incomplete -> SourceCorrupt et Io -> StorageFailure avec message_key conservé.
- Produces: pub impl From<SourceGuardError> for ImportError.

- [ ] **Step 1: Écrire le test rouge**

  Tests rejects_lrcat_lock_before_copy, same_lrcat_lock_maps_to_same_source_locked_code, rejects_active_catalog_before_copy, copies_catalog_and_associated_root_to_workspace et verify_source_unchanged_after_copy. Vérifier que le même fichier .lrcat.lock déclenche le même code SourceLocked dans le core et l’importeur, sans écriture source.

- [ ] **Step 2: Exécuter le test pour vérifier l’échec**

  Run: cargo test -p rawzelia-lightroom-import --test source_safety -- rejects_lrcat_lock_before_copy --exact

  Expected: FAIL avec une erreur de compilation indiquant que validate_source_guard, map_source_guard_error ou WorkspaceCopy n’est pas relié.

- [ ] **Step 3: Implémentation minimale de l’inspection et de la copie**

  Appeler d’abord validate_source_guard(path). Mapper SourceGuardError vers ImportError, avec le même code stable pour le même .lrcat.lock. Limiter source.rs à l’inspection complémentaire, la copie read-only du .lrcat et du répertoire associé, les fingerprints source/copie et WorkspaceCopy. Ne pas réimplémenter la politique active/verrouillée du core.

- [ ] **Step 4: Exécuter le test pour vérifier la réussite**

  Run: cargo test -p rawzelia-lightroom-import --test source_safety

  Expected: PASS, avec source inchangée et copie vérifiée.

- [ ] **Step 5: Commit**

  ~~~bash
  git add crates/rawzelia-lightroom-import
  git commit -m "feat: guard and copy Lightroom sources safely"
  ~~~

### Task 3: Adaptateur latest, manifest exact et golden externe, semaines 10-11

**Files:**
- Create: crates/rawzelia-lightroom-import/src/schema.rs
- Create: crates/rawzelia-lightroom-import/src/adapter.rs
- Create: crates/rawzelia-lightroom-import/src/mapping.rs
- Create: crates/rawzelia-lightroom-import/src/fixture_preflight.rs
- Create: crates/rawzelia-lightroom-import/tests/fixture_preflight.rs
- Create: crates/rawzelia-lightroom-import/tests/golden_latest_catalog.rs
- Create: tests/fixtures/lightroom/manifest.toml
- Modify: crates/rawzelia-lightroom-import/src/lib.rs

**Interfaces:**
- Consumes: WorkspaceCopy, LightroomSchemaId, ImportStatus, FieldProvenance, ImportedRecord et les chemins read-only de Task 2.
- Produces: pub struct FixtureManifest { pub fixture_root_env: String, pub latest: CatalogFixture, pub legacy: std::collections::BTreeMap<String, CatalogFixture>, pub cases: std::collections::BTreeMap<String, FixtureCase> }.
- Produces: pub struct CatalogFixture { pub schema_id: LightroomSchemaId, pub catalog_path: std::path::PathBuf, pub associated_root: Option<std::path::PathBuf>, pub expected_counts: Option<std::path::PathBuf>, pub expected_source_ids: Option<std::path::PathBuf>, pub reference_jpeg: Option<std::path::PathBuf> }.
- Produces: pub struct FixtureCase { pub catalog_path: std::path::PathBuf, pub associated_root: Option<std::path::PathBuf>, pub lock_path: Option<std::path::PathBuf>, pub preset_path: Option<std::path::PathBuf>, pub missing_volume_path: Option<std::path::PathBuf> }.
- Produces: pub const FIXTURE_ROOT_ENV: &str = "RAWZELIA_FIXTURES_DIR".
- Produces: pub fn fixture_root_from_env() -> Result<std::path::PathBuf, ImportError>; elle lit uniquement RAWZELIA_FIXTURES_DIR et refuse une variable absente ou vide.
- Produces: pub fn load_fixture_manifest(manifest_path: &std::path::Path, fixture_root: &std::path::Path) -> Result<FixtureManifest, ImportError>; les entrées latest, legacy et cases sont résolues sous cette unique racine.
- Produces: pub struct SourceRecord { pub source_id: String, pub fields: Vec<SourceField>, pub relationships: Vec<SourceRelationship> }.
- Produces: pub struct SourceField { pub path: String, pub value: SourceValue }.
- Produces: pub enum SourceValue { Text(String), Integer(i64), Decimal(f64), Boolean(bool), Path(std::path::PathBuf), Bytes(Vec<u8>), Null }.
- Produces: pub trait ImportRecordSink { fn push(&mut self, record: ImportedRecord) -> Result<(), ImportError>; }.
- Produces: pub trait LightroomAdapter { fn schema_id(&self) -> LightroomSchemaId; fn can_read(&self, workspace: &WorkspaceCopy) -> Result<bool, ImportError>; fn dry_run(&self, workspace: &WorkspaceCopy) -> Result<ImportReport, ImportError>; fn emit_records(&self, workspace: &WorkspaceCopy, sink: &mut dyn ImportRecordSink) -> Result<(), ImportError>; }.
- Produces: pub struct LatestCatalogAdapter et pub fn select_adapter(workspace: &WorkspaceCopy) -> Result<Box<dyn LightroomAdapter>, ImportError>.
- Produces: pub fn map_catalog_record(row: &SourceRecord, schema: &LightroomSchemaId) -> Result<ImportedRecord, ImportError>.

Le fichier suivi tests/fixtures/lightroom/manifest.toml a exactement les clés suivantes; les chemins désignent des fichiers externes sous RAWZELIA_FIXTURES_DIR, jamais des binaires commités dans le dépôt :

~~~toml
[manifest]
fixture_root_env = "RAWZELIA_FIXTURES_DIR"

[latest]
schema_id = "lightroom-latest-corpus"
catalog_path = "latest/catalog.lrcat"
associated_root = "latest/catalog.lrdata"
expected_counts = "latest/expected-counts.toml"
expected_source_ids = "latest/source-ids.txt"
reference_jpeg = "latest/reference/asset-0001.jpg"

[legacy]
# Hardening ajoute ici des entrées après fixtures et tests dédiés.

[cases.locked]
catalog_path = "cases/locked/catalog.lrcat"
associated_root = "cases/locked/catalog.lrdata"
lock_path = "cases/locked/catalog.lrcat.lock"

[cases.corrupt]
catalog_path = "cases/corrupt/catalog.lrcat"
associated_root = "cases/corrupt/catalog.lrdata"

[cases.missing-lrdata]
catalog_path = "cases/missing-lrdata/catalog.lrcat"

[cases.invalid-preset]
catalog_path = "cases/invalid-preset/catalog.lrcat"
associated_root = "cases/invalid-preset/catalog.lrdata"
preset_path = "cases/invalid-preset/invalid.lrtemplate"

[cases.missing-volume]
catalog_path = "cases/missing-volume/catalog.lrcat"
associated_root = "cases/missing-volume/catalog.lrdata"
missing_volume_path = "cases/missing-volume/absent-volume"
~~~

Les fichiers expected-counts.toml et source-ids.txt sont écrits par l’acquisition du corpus utilisateur sous l’unique racine RAWZELIA_FIXTURES_DIR; le manifest suivi reste exact sans inventer les IDs Adobe. Le premier contient les sept clés assets, folders, collections, collection_sets, smart_collections, stacks et virtual_copies, avec assets = 100 pour le flux prototype. Le second contient un source_id normalisé par ligne. Le préflight exige le catalogue latest, les sidecars d’attentes et le JPEG de référence; il exige un preset uniquement pour le cas invalid-preset, un lock_path pour le cas locked et missing_volume_path pour le cas missing-volume. L’absence de associated_root dans cases.missing-lrdata est une règle explicite: le .lrdata attendu doit être absent et l’import continue avec le statut ignored. Les entrées legacy utilisent le même modèle et la même racine; hardening modifie et étend ce manifest, sans redéfinir FixtureManifest. Le plan ne prétend pas fournir un catalogue Adobe réel: les dossiers binaires latest/ et cases/ ne sont ni créés ni embarqués dans Git.

- [ ] **Step 1: Écrire les tests rouges**

  Dans fixture_preflight.rs, écrire manifest_preflight_requires_external_catalog, manifest_preflight_rejects_missing_case_path, manifest_preflight_enforces_missing_lrdata_absence et manifest_accepts_legacy_entries_without_selecting_them. Dans golden_latest_catalog.rs, écrire latest_fixture_selects_only_latest_adapter_and_emits_expected_counts et unknown_schema_is_rejected_without_legacy_claim. Vérifier que le manifest est lu depuis le dépôt, que les catalogues sont résolus uniquement via RAWZELIA_FIXTURES_DIR et que les comptages/IDs attendus sont contrôlés.

- [ ] **Step 2: Exécuter les tests pour vérifier l’échec**

  Run: test -n "${RAWZELIA_FIXTURES_DIR:-}" && cargo test -p rawzelia-lightroom-import --test fixture_preflight -- manifest_preflight_requires_external_catalog --exact

  Expected: FAIL avec une erreur de compilation indiquant que FixtureManifest, fixture_root_from_env ou le loader du manifest n’est pas défini; après bootstrap, l’absence des fichiers externes doit rester une erreur explicite, jamais un succès vide.

- [ ] **Step 3: Implémentation minimale de l’adaptateur latest et du préflight**

  Parser le manifest exact, vérifier que manifest.fixture_root_env vaut RAWZELIA_FIXTURES_DIR, résoudre tous ses chemins sous cette racine unique et refuser toute entrée manquante. Lire seulement les tables et colonnes observées dans le catalogue latest du corpus. Normaliser chemins, volumes, dossiers, dates, EXIF/IPTC/GPS, captions, notes, droits, étoiles, drapeaux, labels couleur et mots-clés hiérarchiques, puis produire les relations collections, ensembles, collections dynamiques, piles et copies virtuelles. Préserver source_id, schéma et fingerprint; conserver tout champ non reconnu en UnknownField sous lightroom.*. Ne sélectionner aucun adaptateur legacy.

- [ ] **Step 4: Exécuter les tests pour vérifier la réussite**

  Run: test -n "${RAWZELIA_FIXTURES_DIR:-}" && cargo test -p rawzelia-lightroom-import --tests

  Expected: PASS avec le corpus externe préparé, comptages et IDs du manifest, sélection exclusive de lightroom-latest-corpus et rejet d’un schéma non enregistré.

- [ ] **Step 5: Commit**

  ~~~bash
  git add crates/rawzelia-lightroom-import tests/fixtures/lightroom/manifest.toml
  git commit -m "feat: import latest Lightroom catalog schema"
  ~~~

### Task 4: Dry-run, rapport et plan métier, semaines 11-12

**Files:**
- Create: crates/rawzelia-app/src/lightroom_import.rs
- Modify: crates/rawzelia-app/src/lib.rs
- Modify: crates/rawzelia-lightroom-import/src/report.rs
- Modify: crates/rawzelia-lightroom-import/src/lib.rs
- Test: crates/rawzelia-app/tests/lightroom_import_dry_run.rs

**Interfaces:**
- Consumes: inspect_source, prepare_workspace, select_adapter, LightroomAdapter::dry_run, ImportReport, ImportCounts, Fingerprint et `rawzelia_contract::commands::ReferenceJpegSelection`.
- Produces: pub struct LightroomImportRequest { pub source_path: std::path::PathBuf, pub workspace_root: std::path::PathBuf, pub target_catalog: std::path::PathBuf, pub reference_jpeg: Option<ReferenceJpegSelection> }.
- Produces: pub struct ImportPlan { pub report_id: ImportReportId, pub source: LightroomSourceIdentity, pub workspace: WorkspaceCopy, pub expected_counts: ImportCounts, pub expected_source_ids: Vec<String>, pub target_catalog: std::path::PathBuf, pub reference_jpeg: Option<ReferenceJpegSelection> }.
- Produces: pub fn run_lightroom_dry_run(request: &LightroomImportRequest) -> Result<ImportReport, ImportError>.
- Produces: pub fn create_import_plan(report: &ImportReport, target_catalog: &std::path::Path, reference_jpeg: Option<ReferenceJpegSelection>) -> Result<ImportPlan, ImportError>.

- [ ] **Step 1: Écrire le test rouge**

  Écrire dry_run_reports_schema_counts_ids_fingerprints_and_statuses_without_mutation et reference_jpeg_selection_is_carried_into_plan. Vérifier schéma latest, comptages, IDs source, fingerprints, statuts par champ, chemins absents, volume absent et éléments ignorés. Comparer les fingerprints avant et après et refuser un plan dont la copie ou la sélection JPEG ne correspond plus au rapport.

- [ ] **Step 2: Exécuter le test pour vérifier l’échec**

  Run: cargo test -p rawzelia-app --test lightroom_import_dry_run -- dry_run_reports_schema_counts_ids_fingerprints_and_statuses_without_mutation --exact

  Expected: FAIL avec une erreur de compilation indiquant que run_lightroom_dry_run ou ImportPlan n’est pas défini.

- [ ] **Step 3: Implémentation minimale du dry-run et du rapport**

  Orchestrer inspection, copie, sélection d’adaptateur et rapport sans transaction d’écriture. Vérifier la sélection JPEG explicite, son fingerprint et son source_id, puis la recopier dans ImportPlan. Séparer importé, approximé, préservé, non supporté et ignoré; inclure les issues par source, provenance et fingerprint observé. create_import_plan ne modifie encore aucune base.

- [ ] **Step 4: Exécuter le test pour vérifier la réussite**

  Run: cargo test -p rawzelia-app --test lightroom_import_dry_run

  Expected: PASS, avec source inchangée et plan confirmable seulement si le fingerprint reste identique.

- [ ] **Step 5: Commit**

  ~~~bash
  git add crates/rawzelia-app crates/rawzelia-lightroom-import
  git commit -m "feat: add Lightroom dry-run orchestration"
  ~~~

### Task 5: Transaction SQLite séparée, vérification et rollback, semaines 12-13

**Files:**
- Create: crates/rawzelia-storage/src/lightroom_batch.rs
- Modify: crates/rawzelia-storage/src/lib.rs
- Create: crates/rawzelia-app/src/lightroom_commit.rs
- Modify: crates/rawzelia-app/src/lightroom_import.rs
- Test: crates/rawzelia-storage/tests/lightroom_batch.rs

**Interfaces:**
- Consumes: ImportPlan, ImportedRecord, LightroomSourceIdentity, ImportCounts, ImportedField<T>, Fingerprint, ImportedArtifact et l’enum `rawzelia_storage::StorageError` du core.
- Produces: pub trait LightroomBatchStore { fn begin_import(&mut self, source: &LightroomSourceIdentity, expected: &ImportCounts) -> Result<ImportBatchId, StorageError>; fn append_record(&mut self, batch_id: &ImportBatchId, record: &ImportedRecord) -> Result<(), StorageError>; fn append_artifact(&mut self, batch_id: &ImportBatchId, artifact: &ImportedArtifact) -> Result<(), StorageError>; fn verify_import(&self, batch_id: &ImportBatchId, expected: &ImportCounts, expected_source_ids: &[String]) -> Result<BatchVerification, StorageError>; fn commit_import(&mut self, batch_id: &ImportBatchId) -> Result<(), StorageError>; fn rollback_import(&mut self, batch_id: &ImportBatchId) -> Result<(), StorageError>; }.
- Produces: impl From<StorageError> for ImportError; la conversion conserve le code de stockage et renseigne message_key, source_id et field_path quand le contexte les fournit.
- Produces: pub struct BatchVerification { pub counts_match: bool, pub ids_match: bool, pub fingerprints_match: bool }.
- Produces: pub struct ImportSummary { pub batch_id: ImportBatchId, pub verification: BatchVerification }.
- Produces: pub fn open_import_catalog(path: &std::path::Path) -> Result<Box<dyn LightroomBatchStore>, StorageError>.
- Produces: pub fn commit_lightroom_import(plan: &ImportPlan, artifacts: &[ImportedArtifact]) -> Result<ImportSummary, ImportError>.
- Produces: pub fn rollback_lightroom_import(batch_id: ImportBatchId) -> Result<(), ImportError>.

- [ ] **Step 1: Écrire le test rouge**

  Tests imports_into_separate_catalog_and_verifies_counts, appends_reference_artifact_and_rolls_back, rejects_id_or_fingerprint_mismatch, rolls_back_only_imported_batch et never_writes_source_catalog. Comparer comptages, IDs et fingerprints au plan, provoquer un écart et vérifier après rollback l’absence des records et de l’artefact JPEG du lot, avec présence intacte d’un lot antérieur.

- [ ] **Step 2: Exécuter le test pour vérifier l’échec**

  Run: cargo test -p rawzelia-storage --test lightroom_batch -- imports_into_separate_catalog_and_verifies_counts --exact

  Expected: FAIL avec une erreur de compilation indiquant que LightroomBatchStore ou open_import_catalog n’est pas défini.

- [ ] **Step 3: Implémentation minimale du store et des commandes transactionnelles**

  Ouvrir uniquement le catalogue Rawzelia cible en écriture. Enregistrer batch_id, fingerprint source, schéma et comptages attendus. Insérer les records, puis chaque ImportedArtifact fourni par associate_reference_jpeg via append_artifact dans la même transaction SQLite/WAL. Vérifier comptages, IDs et fingerprints avant commit, puis publier le lot. En cas d’erreur ou de rollback, supprimer records et artefacts portant ce batch_id; la source Lightroom reste hors de la connexion d’écriture.

- [ ] **Step 4: Exécuter les tests pour vérifier la réussite**

  Run: cargo test -p rawzelia-storage --test lightroom_batch

  Expected: PASS, avec commit valide, rollback ciblé, mismatch refusé et source inchangée.

- [ ] **Step 5: Commit**

  ~~~bash
  git add crates/rawzelia-storage crates/rawzelia-app
  git commit -m "feat: import Lightroom records transactionally"
  ~~~

### Task 6: XMP, presets, previews, Develop et JPEG de référence, semaines 13-15

**Files:**
- Create: crates/rawzelia-lightroom-import/src/artifacts.rs
- Create: crates/rawzelia-lightroom-import/src/develop.rs
- Modify: crates/rawzelia-lightroom-import/src/adapter.rs
- Modify: crates/rawzelia-lightroom-import/src/lib.rs
- Test: crates/rawzelia-lightroom-import/tests/lightroom_artifacts.rs

**Interfaces:**
- Consumes: WorkspaceCopy, ImportedField<T>, ImportedArtifact, ImportedDevelop, FieldProvenance, ImportStatus et `rawzelia_contract::commands::ReferenceJpegSelection`; aucune écriture SQLite.
- Produces: pub fn read_xmp_sidecar(path: &std::path::Path, schema: &LightroomSchemaId) -> Result<Vec<ImportedField<DevelopValue>>, ImportError>.
- Produces: pub fn read_lrtemplate(path: &std::path::Path, schema: &LightroomSchemaId) -> Result<ImportedArtifact, ImportError>.
- Produces: pub fn read_preview_set(workspace: &WorkspaceCopy, source_id: &str) -> Result<ImportedPreviews, ImportError>.
- Produces: pub fn preserve_develop_fields(workspace: &WorkspaceCopy, source_id: &str) -> Result<ImportedDevelop, ImportError>.
- Produces in `reference.rs`: pub fn validate_reference_jpeg(selection: &ReferenceJpegSelection, candidates: &[std::path::PathBuf]) -> Result<(), ImportError>; elle refuse une sélection absente des candidats ou une ambiguïté non résolue.
- Produces in `reference.rs`: pub fn verify_reference_jpeg_fingerprint(selection: &ReferenceJpegSelection) -> Result<(), ImportError>; elle recalcule le SHA-256 et refuse toute divergence.
- Produces: pub fn associate_reference_jpeg(selection: &ReferenceJpegSelection) -> Result<ImportedArtifact, ImportError>; elle construit un artefact typé sans ouvrir le store.

`reference.rs` ne contient que la validation et le fingerprint du `ReferenceJpegSelection` canonique; il ne produit ni ne redéfinit `ReferenceJpegSelection` ou un payload de commande. Le refus explicite est représenté par `None` au niveau de `LightroomImportRequest`/`InspectLightroomCommand`; une liste de candidats ambiguë sans sélection est refusée par `validate_reference_jpeg`. `associate_reference_jpeg` construit l’artefact avec la provenance `lightroom.reference_preview.*`. Task 5 l’envoie à `append_artifact` dans la transaction; rollback supprime cet artefact avec le lot.

- [ ] **Step 1: Écrire les tests rouges**

  Tests reads_xmp_and_keeps_field_provenance, invalid_preset_is_preserved_and_classified, missing_lrdata_is_non_fatal, develop_is_kept_under_lightroom_namespace, unknown_plugin_data_is_preserved, reference_jpeg_requires_explicit_confirmation, ambiguous_reference_jpeg_is_rejected, reference_jpeg_validation_uses_contract_type, reference_jpeg_fingerprint_is_recorded et reference_jpeg_artifact_is_typed_for_transaction. Vérifier previews embedded, standard et Smart Preview quand présents, absence de .lrdata sans erreur globale, historique et réglages Develop conservés, serde_json::Value uniquement pour les plugins inconnus et le fingerprint de l’artefact.

- [ ] **Step 2: Exécuter le test pour vérifier l’échec**

  Run: cargo test -p rawzelia-lightroom-import --test lightroom_artifacts -- reference_jpeg_artifact_is_typed_for_transaction --exact

  Expected: FAIL avec une erreur de compilation indiquant que validate_reference_jpeg, verify_reference_jpeg_fingerprint, associate_reference_jpeg ou ImportedArtifact n’est pas défini.

- [ ] **Step 3: Implémentation minimale des lecteurs et de l’association consentie**

  Lire XMP et .lrtemplate en lecture seule. Classer un preset invalide unsupported tout en conservant son payload et sa provenance. Conserver les previews disponibles; classer l’absence de .lrdata ignored. Ranger historique et réglages Develop sous lightroom.history.* et lightroom.develop.*; mapper uniquement les réglages explicitement supportés avec le statut approprié. Conserver les données plugin inconnues sous lightroom.unknown.*. L’association JPEG appelle uniquement la validation et le fingerprint de `reference.rs` sur le `ReferenceJpegSelection` canonique; elle ne déduit aucune association et n’écrit pas dans SQLite.

- [ ] **Step 4: Exécuter les tests pour vérifier la réussite**

  Run: cargo test -p rawzelia-lightroom-import --test lightroom_artifacts

  Expected: PASS, avec artefacts et statuts attendus, choix explicite, ambiguïté refusée et aucune promesse de rendu Adobe identique.

- [ ] **Step 5: Commit**

  ~~~bash
  git add crates/rawzelia-lightroom-import
  git commit -m "feat: preserve Lightroom artifacts and reference previews"
  ~~~

### Task 7: Inspecteur Tauri et rapport utilisateur en français, semaines 15-16

**Files:**
- Create: crates/rawzelia-desktop/src/lightroom_import.rs
- Create: crates/rawzelia-desktop/src/i18n/fr.json
- Modify: crates/rawzelia-desktop/src/lib.rs
- Test: crates/rawzelia-desktop/tests/lightroom_import_commands.rs
- Modify: crates/rawzelia-app/src/lib.rs
- Modify: crates/rawzelia-app/src/lightroom_import.rs
- Consume: crates/rawzelia-desktop/ui/src/bridge.ts (builders Lightroom produits par le core)

**Interfaces:**
- Consumes directly from the core-owned `rawzelia-contract/src/commands.rs`: `AppCommand`, `InspectLightroomCommand`, `ExecuteLightroomCommand`, `RollbackLightroomCommand` et `ReferenceJpegSelection`; `rawzelia-app/src/lightroom_import.rs` ne redéfinit aucun de ces types. Consumes aussi depuis `crates/rawzelia-desktop/ui/src/bridge.ts` les builders `buildInspectLightroomCommand`, `buildExecuteLightroomCommand` et `buildRollbackLightroomCommand` produits par le core.
- Produces in the existing `rawzelia-app/src/lightroom_import.rs`: `pub fn inspect_lightroom_command(command: InspectLightroomCommand) -> Result<ImportReport, ImportError>`, `pub fn execute_lightroom_command(command: ExecuteLightroomCommand, plan: &ImportPlan, artifacts: &[ImportedArtifact]) -> Result<ImportSummary, ImportError>` et `pub fn rollback_lightroom_command(command: RollbackLightroomCommand) -> Result<(), ImportError>`. L’app transmet directement `command.reference_jpeg` au `LightroomImportRequest`, puis au `ImportPlan`, valide le même `ReferenceJpegSelection` et son fingerprint avant `associate_reference_jpeg`/`append_artifact`; elle mappe `batch_id` vers `ImportBatchId` pour le rollback existant.
- Produces: pub enum LightroomImportEvent { ReportReady { report: ImportReport }, Progress { processed: u64, total: Option<u64> }, Completed { summary: ImportSummary }, Failed { error_code: String } }.
- Produces: #[tauri::command] pub async fn inspect_lightroom_import(command: InspectLightroomCommand, state: State<'_, DesktopState>) -> Result<ImportReport, DesktopError>.
- Produces: #[tauri::command] pub async fn execute_lightroom_import(command: ExecuteLightroomCommand, state: State<'_, DesktopState>) -> Result<ImportSummary, DesktopError>.
- Produces: #[tauri::command] pub async fn rollback_lightroom_import(command: RollbackLightroomCommand, state: State<'_, DesktopState>) -> Result<(), DesktopError>.

- [ ] **Step 1: Écrire le test rouge**

  Tests `contract_app_desktop_commands_round_trip`, `app_command_carries_reference_jpeg`, `desktop_rejects_ambiguous_reference_jpeg`, `desktop_allows_reference_jpeg_decline`, `inspect_execute_append_artifact_rollback_preserves_reference_type_and_fingerprint`, `execute_command_commits_verified_transaction`, `desktop_reference_jpeg_is_removed_after_rollback`, `report_view_exposes_counts_and_statuses` et `french_strings_cover_import_states`. Vérifier la sérialisation inter-crates des commandes canoniques, l’utilisation des builders TS du core, puis le trajet inspect → execute → `append_artifact` → rollback avec le même `ReferenceJpegSelection` et le même fingerprint; vérifier le refus d’une ambiguïté, le commit uniquement après vérification et le rollback ciblé. Aucun serveur REST n’est créé.

- [ ] **Step 2: Exécuter le test pour vérifier l’échec**

  Run: cargo test -p rawzelia-desktop --test lightroom_import_commands -- inspect_execute_append_artifact_rollback_preserves_reference_type_and_fingerprint --exact

  Expected: FAIL avec une erreur de compilation indiquant que l’orchestration app/Tauri ou le trajet inter-crates inspect → execute → append_artifact → rollback n’est pas encore câblé autour des commandes et builders canoniques du core.

- [ ] **Step 3: Implémentation minimale des commandes et événements**

  Consommer sans modification les commandes canoniques et les builders TS produits par le core. Relayer `InspectLightroomCommand` vers `LightroomImportRequest` en transmettant directement `command.reference_jpeg`, puis publier le même `AppCommand` vers `AppRuntime::execute` dans `rawzelia-app/src/lib.rs`. Résoudre l’identifiant de rapport de `ExecuteLightroomCommand` vers l’`ImportPlan` déjà produit par le dry-run, vérifier le consentement et le fingerprint du même `ReferenceJpegSelection`, exécuter `commit_lightroom_import(plan, artifacts)` avec `append_artifact`, puis exposer `rollback_lightroom_command` via le même bus et le même `batch_id`. Enregistrer les commandes dans `invoke_handler` de Tauri. Ajouter les chaînes françaises dans `fr.json`; ne copier aucune aide, infobulle, message ou autre texte propriétaire Adobe. Afficher distinctement les données exactes, approximées, préservées et non supportées.

- [ ] **Step 4: Exécuter les tests pour vérifier la réussite**

  Run: cargo test -p rawzelia-desktop --test lightroom_import_commands

  Expected: PASS, avec sérialisation identique entre contract, app et desktop, choix JPEG explicite, même type et fingerprint jusqu’à `append_artifact`, transaction vérifiée, rollback du seul lot et aucune route HTTP ni serveur REST ajouté.

- [ ] **Step 5: Commit**

  ~~~bash
  git add crates/rawzelia-contract/src/commands.rs crates/rawzelia-app/src/lib.rs crates/rawzelia-app/src/lightroom_import.rs crates/rawzelia-desktop
  git commit -m "feat: expose Lightroom import inspector"
  ~~~

### Task 8: Golden end-to-end, association JPEG et gate semaine 16

**Files:**
- Create: crates/rawzelia-lightroom-import/tests/lightroom_import_e2e.rs
- Create: crates/rawzelia-app/tests/lightroom_import_acceptance.rs
- Modify: tests/fixtures/lightroom/manifest.toml

Les cas locked, corrupt, missing-lrdata, invalid-preset et missing-volume sont des chemins de fixtures externes définis dans le manifest; aucun répertoire vide ni catalogue Adobe réel n’est ajouté au dépôt.

**Interfaces:**
- Consumes: les commandes d’application, l’adaptateur latest, LightroomBatchStore, les fixtures externes et le manifest des Tasks 1 à 7.
- Produces: pub struct AcceptanceSummary { pub assets: u64, pub rollback_ok: bool, pub source_unchanged: bool, pub reference_jpeg_rolled_back: bool }.
- Produces: pub fn run_latest_catalog_acceptance() -> Result<AcceptanceSummary, TestError> dans le harnais de test.

- [ ] **Step 1: Écrire le test rouge**

  Écrire latest_catalog_round_trip_matches_manifest_and_rolls_back_cleanly, reference_jpeg_is_rolled_back_with_batch et fixture_failures_are_reported_without_source_mutation. Exécuter dry-run, import transactionnel, vérification comptages/IDs/fingerprints, association JPEG explicitement consentie, puis rollback. Couvrir locked, corrupt, .lrdata absent, preset invalide, volume absent et plugin inconnu. Vérifier que seul lightroom-latest-corpus est annoncé et qu’aucun test ne compare à un rendu Adobe identique.

- [ ] **Step 2: Exécuter le test pour vérifier l’échec**

  Run: test -n "${RAWZELIA_FIXTURES_DIR:-}" && cargo test -p rawzelia-lightroom-import --test lightroom_import_e2e -- latest_catalog_round_trip_matches_manifest_and_rolls_back_cleanly --exact

  Expected: FAIL avec une erreur de compilation indiquant que run_latest_catalog_acceptance ou AcceptanceSummary n’est pas défini.

- [ ] **Step 3: Implémentation minimale du harnais golden et des assertions de reprise**

  Charger les attentes exclusivement depuis manifest.toml, résoudre les fichiers via RAWZELIA_FIXTURES_DIR, comparer comptages et IDs avant/après, vérifier fingerprints et absence de mutation du .lrcat, .lrdata, XMP et originaux. Vérifier que le rollback supprime le JPEG de référence et les records du seul lot, tout en conservant un lot antérieur. Toute version antérieure reste rejetée jusqu’à l’existence d’un adaptateur, d’une entrée de manifest et de tests dédiés.

- [ ] **Step 4: Exécuter le gate complet de la semaine 16**

  Run: test -n "${RAWZELIA_FIXTURES_DIR:-}" && cargo test --workspace

  Expected: PASS pour les tests unitaires, golden, verrouillage, corruption, .lrdata absent, preset invalide, volume absent, données inconnues, association JPEG, transaction et rollback.

  Run: test -n "${RAWZELIA_FIXTURES_DIR:-}" && cargo test -p rawzelia-lightroom-import --release --test lightroom_import_e2e -- latest_catalog_round_trip_matches_manifest_and_rolls_back_cleanly --exact

  Expected: PASS et au moins un test exécuté, sans --ignored, avec le flux de 100 NEF sur Apple Silicon M1 16 Go; l’extraction reste progressive pour les archives de 10 000 à 50 000 photos.

- [ ] **Step 5: Commit**

  ~~~bash
  git add crates/rawzelia-lightroom-import crates/rawzelia-app tests/fixtures/lightroom/manifest.toml
  git commit -m "test: validate Lightroom import safety and rollback"
  ~~~

## Répartition des semaines

- Semaines 9-10 : contrats canoniques, garde source, copie workspace et fingerprints.
- Semaines 10-11 : manifest externe, préflight, inspection du schéma latest et golden adapter.
- Semaines 11-12 : dry-run, rapport, volumes absents, statuts et plan confirmable.
- Semaines 12-13 : catalogue Rawzelia séparé, vérification transactionnelle et rollback ciblé.
- Semaines 13-15 : XMP, .lrtemplate, previews, Smart Previews, historique, Develop, plugin inconnu et JPEG de référence.
- Semaines 15-16 : inspecteur Tauri français, golden end-to-end, gate M1 et durcissement de la reprise.
- Après la semaine 16 : adaptateurs de versions Lightroom antérieures seulement après fixtures golden, tests et rapport de compatibilité; ils ne sont pas inclus dans ce plan.

## Definition of Done

- [ ] Le core produit et exporte LightroomSchemaId, ImportStatus, FieldProvenance, ImportedField et UnknownField depuis rawzelia-contract/src/lightroom_import.rs; ce plan les consomme sans les redéfinir.
- [ ] WorkspaceCopy, ImportError, LightroomAdapter, ImportPlan et LightroomBatchStore sont produits par leurs owners avec les signatures indiquées; IngestPlan reste réservé à la carte SD.
- [ ] Les records connus sont des sous-types Rust; seul UnknownField.raw_value utilise serde_json::Value, sous lightroom.* avec provenance.
- [ ] Le manifest exact est suivi dans tests/fixtures/lightroom/manifest.toml; RAWZELIA_FIXTURES_DIR préflighte les artefacts externes sans catalogue Adobe embarqué.
- [ ] Le catalogue latest du corpus est détecté par son adaptateur dédié, avec comptages, IDs et fingerprints vérifiés.
- [ ] Un catalogue actif, verrouillé ou corrompu est refusé ou rapporté sans écriture source; le .lrcat, .lrdata, XMP et les originaux restent inchangés.
- [ ] Le dry-run distingue exact, approximated, preserved_only, unsupported et ignored par champ ou artefact.
- [ ] Chemins, volumes, dossiers, dates, EXIF/IPTC/GPS, captions, notes, droits, étoiles, drapeaux, labels, mots-clés hiérarchiques, collections, ensembles, smart collections, piles, copies virtuelles, XMP, .lrtemplate, previews, historique, Develop et champs inconnus sont couverts par les fixtures applicables.
- [ ] Un JPEG de référence Lightroom n’est associé qu’après consentement explicite, avec fingerprint; ambiguïté et sélection invalide sont refusées, et rollback supprime l’association du lot.
- [ ] L’import écrit uniquement dans un catalogue Rawzelia séparé, vérifie comptages/IDs/fingerprints et commit ou rollback le lot atomique.
- [ ] `AppCommand`, `InspectLightroomCommand`, `ExecuteLightroomCommand`, `RollbackLightroomCommand` et `ReferenceJpegSelection` sont définis une seule fois dans `rawzelia-contract/src/commands.rs`; `reference.rs` ne contient que validation/fingerprint sur le type canonique; `rawzelia-app/src/lightroom_import.rs` et Tauri consomment les commandes et builders du core, transportent le même `reference_jpeg` jusqu’à `append_artifact` et au rollback, et aucun serveur REST n’est ajouté.
- [ ] L’inspecteur utilise des commandes et événements Tauri typés, reste en français, externalise ses chaînes et n’ajoute aucune API REST.
- [ ] Le gate normal et release exécute réellement le golden sans --ignored; le flux 100 NEF passe sur M1 16 Go et aucune parité de rendu Adobe n’est promise.
