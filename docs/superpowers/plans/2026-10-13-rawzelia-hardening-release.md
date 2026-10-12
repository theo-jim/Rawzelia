# Rawzelia v0.1 Hardening and Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Durcir Rawzelia v0.1 pendant les semaines 17 à 20 afin de valider les anciens catalogues Lightroom, la récupération et le rollback, l’échelle 10 000 à 50 000 photos, l’accessibilité, la conformité Community/Pro et la publication signée macOS Apple Silicon.

**Architecture:** Le plan consomme les contrats déjà définis par les plans socle et import conceptuels. Il conserve le modular monolith Rust, ajoute les adaptateurs Lightroom anciens derrière `rawzelia-lightroom-import`, garde le CPU comme oracle d’image et traite wgpu comme proxy optionnel. Les tâches de durcissement n’ajoutent aucune fonction Pro et ne livrent pas Windows.

**Tech Stack:** Rust stable et Cargo workspace, Tauri 2/WebView, SQLite WAL, LibRaw, Little CMS, wgpu/Metal optionnel, `serde`, `quick-xml`, `cargo-deny`, SPDX, `codesign`, `xcrun notarytool`, `xcrun stapler`, `spctl`, tests Rust unitaires et d’intégration, fixtures Lightroom versionnées et corpus NEF D750 hors dépôt si leur redistribution n’est pas autorisée.

**Spec:** `docs/superpowers/specs/2026-10-12-rawzelia-v0.1-design.md`

## Global Constraints

- Cible publiée : `macOS sur Apple Silicon`; aucune tâche ne livre Windows.
- Formats RAW v0.1 : `fichiers RAW NEF issus du Nikon D750`.
- Objectifs de référence : `Nikon 16-35 f/4G VR`, `Nikon 24-70 f/2.8E ED VR`, `Sigma 50 f/1.4 Art`, `Samyang 85 f/1.4 manuel`.
- Import Lightroom : copie, sauvegarde ou export seulement ; aucune écriture dans `.lrcat`, `.lrdata`, XMP ou originaux ; aucune mise à niveau Adobe préalable imposée.
- Adaptateurs : le premier adaptateur cible le catalogue le plus récent du corpus utilisateur ; les versions antérieures sont ajoutées seulement après fixtures, dry-run, comptages, corruption, verrouillage et rollback validés.
- Contrats Lightroom canoniques : `LightroomSchemaId`, `ImportStatus` et `FieldProvenance` sont produits par `rawzelia-contract/src/lightroom_import.rs` dans le plan core; `WorkspaceCopy`, `ImportError`, `LightroomAdapter`, `ImportPlan` et `LightroomBatchStore` sont produits par leurs owners du plan import. Tous sont seulement consommés ici. Les statuts exacts sont `exact`, `approximated`, `preserved_only`, `unsupported` et `ignored`; aucun type concurrent `RecoveryStatus` n’est introduit.
- `ImportPlan` est réservé au flux Lightroom; le flux carte SD reste `rawzelia-io::IngestPlan`, `plan_ingest` et `execute_ingest`. Aucune tâche de ce plan n’accepte l’un à la place de l’autre.
- Contrats socle canoniques : `CatalogStore`, `LibraryQuery`, `RenderRequest` et `JobState` sont produits par le plan core et seulement consommés ici. Les signatures du plan import et du plan core ne sont ni redéfinies ni adaptées implicitement.
- Sidecar : `.rawzelia` complet et versionné ; XMP limité au miroir compatible ; aucun écrasement silencieux en cas de divergence.
- CPU : référence pour décodage, colorimétrie, développement, export et fallback ; GPU limité au proxy interactif ; export final CPU.
- Budget catalogue : ouverture à chaud d’un catalogue de 10 000 photos sous 5 s sans régénération des aperçus ; recherche p95 sous 200 ms sur 10 000 photos et sous 300 ms sur 50 000 entrées réparties sur plusieurs racines.
- Budget UI : réglage visible p95 sous 100 ms sur proxy ; loupe cache chaud p95 sous 400 ms ; grille d’au moins 50 fps sur 500 vignettes ; zoom et pan à 60 fps ; aucun blocage UI supérieur à 500 ms au jalon semaine 8.
- Budget mémoire : moins de 1,5 Go pendant 30 minutes de tri sur 10 000 photos, hors outils de profilage.
- Reprise : 30 cycles interrompus sans perte d’état ; import, sidecars, exports, sauvegardes et migrations atomiques et vérifiés par hash.
- Community : MPL-2.0, autonome, hors ligne, réellement utilisable sans dépôt privé ; les modules Pro privés sont optionnels et absents du build Community.
- Fonctions Pro hors plan : masques, retouches avancées, assistance IA, HDR, panorama, tethering, automatisation, plugins distribués, cloud et conversion avancée des retouches Lightroom.
- Confidentialité : télémétrie absente par défaut ; diagnostics de crash locaux désactivés jusqu’au consentement explicite ; aucune transmission distante dans ce plan. Ces invariants sont vérifiés et documentés, mais ne sont pas des gates bloquants de publication dans ce plan.
- UX : commandes métier communes au menu, bouton, palette et raccourci ; `G`, `E`, `D`, `P`, `X`, `1` à `5` et `Tab` restent configurables ; toutes les chaînes sont externalisées en français avec préparation anglaise.
- Packaging : bundle Community macOS reproductible, signé, notarisé et vérifiable ; secrets Apple uniquement dans l’environnement de release, jamais dans le dépôt.
- Cargo Community : la feature `community` appartient au package `rawzelia-desktop`, avec `default = ["community"]` et `community = []`; les commandes ciblent explicitement ce package et ne passent jamais `--features community` à `--workspace`.
- Fixtures : `RAWZELIA_FIXTURES_DIR` est l’unique racine obligatoire des catalogues Lightroom et corpus NEF non redistribuables. Le plan import reste propriétaire de `FixtureManifest`, `CatalogFixture` et `FixtureCase`; ce plan ne fait que modifier/étendre ce modèle canonique pour les schémas legacy et les cas `active`/`incomplete`, avec les mêmes chemins et `schema_id` dans `tests/fixtures/lightroom/manifest.toml`.
- Rédaction : ce plan ne crée aucun commit; les étapes `Commit` décrivent uniquement l’exécution future du plan.
- Périmètre temporel : semaines 17 à 20 ; aucun redesign du catalogue, du pipeline NEF ou de l’architecture Tauri/egui.

## Review Focus

- Catalogue Lightroom actif, verrouillé, tronqué, corrompu, ancien ou contenant des données de plugin inconnues. Tests propriétaires : `Task 1 / previous_schema_selects_legacy_v1_without_upgrade`, `Task 1 / active_and_incomplete_cases_are_declared_before_consumption`, `Task 2 / import_rejects_locked_or_corrupt_source_and_rolls_back_batch` et `Task 2 / reference_jpeg_artifact_is_removed_with_batch_id`.
- Volume absent, volume amovible déconnecté, permission refusée, chemin Unicode et racines multiples. Test propriétaire : `Task 3 / refresh_locations_preserves_assets_when_volume_disconnects`.
- Archive de 50 000 entrées, cache chaud/froid, génération obsolète et pression mémoire. Test propriétaire : `Task 4 / performance_budgets_m1_16gb_pass`.
- Focus texte, navigation clavier, nom accessible manquant et clé de traduction absente. Tests propriétaires : `Task 5 / keyboard_commands_respect_text_focus_and_locales_are_complete` et `Task 5 / all_required_controls_have_role_name_and_focus_order`.
- Build Community sans dépôt privé, dépendance sous licence interdite et état de release. Tests propriétaires : `Task 7 / community_build_does_not_require_private_repository`, `Task 7 / spdx_manifest_lists_every_distributed_dependency` et `Task 8 / release_manifest_lists_unsigned_signed_and_notarized_artifacts`.

## File Map and Boundaries

- `crates/rawzelia-lightroom-import/src/adapters/legacy.rs` : adaptateurs pour les schémas Lightroom antérieurs au schéma le plus récent du corpus.
- `crates/rawzelia-lightroom-import/src/import_transaction.rs` : validation de copie, journal de lot, transaction, corruption et rollback.
- `crates/rawzelia-storage/src/volume_status.rs` : état des racines et volumes sans modifier les originaux.
- `crates/rawzelia-storage/src/backup.rs` : sauvegarde cohérente du catalogue et des sidecars, validation et restauration.
- `crates/rawzelia-imaging/src/performance.rs` : budgets, chronométrage, génération de résultats et échantillonnage mémoire.
- `crates/rawzelia-contract/src/keymap.rs` et `crates/rawzelia-contract/src/capabilities.rs` : commandes clavier, capacités Community/Pro et contrats versionnés.
- `crates/rawzelia-desktop/src/accessibility.rs`, `crates/rawzelia-desktop/src/i18n.rs` et `crates/rawzelia-desktop/locales/{fr-FR,en-US}.json` : manifeste accessible et chaînes externalisées.
- `crates/rawzelia-app/src/diagnostics.rs` : configuration de confidentialité et diagnostics de crash locaux avec consentement.
- `scripts/release/macos-community.sh`, `scripts/release/verify-macos.sh`, `packaging/macos/` et `.github/workflows/release-macos-community.yml` : build, signature, notarisation, vérification et CI de release.
- `deny.toml`, `LICENSE`, `NOTICE`, `licenses/THIRD_PARTY.md` et `release/` : conformité licences, provenance et manifestes d’artefacts.
- `crates/rawzelia-contract/src/release_contract.rs` : ownership unique des wrappers, manifestes, états d’artefact, rapports de licence et types de publication consommés par les tâches 7 à 9.
- `tests/fixtures/lightroom/` et `tests/matrix/` : manifests et petits fixtures anonymisés ; le corpus NEF et les catalogues utilisateur restent injectés par `RAWZELIA_FIXTURES_DIR`.

## Ownership and Contract Ledger

| Contrat ou type | Producteur unique | Consommateurs dans ce plan |
|---|---|---|
| `LightroomSchemaId`, `ImportStatus`, `FieldProvenance` | Plan core, `rawzelia-contract/src/lightroom_import.rs` | Tasks 1 et 2 ; aucune redéfinition |
| `WorkspaceCopy`, `ImportError` | Plan import Task 2, `workspace.rs` et `errors.rs` | Tasks 1 et 2 ; aucune redéfinition |
| `LightroomAdapter`, `ImportReport`, `ImportRecordSink`, `ImportedRecord`, `ImportCounts`, `ImportBatchId`, `ImportSummary` | Plan import Tasks 1 et 3, fichiers canoniques du plan import | Tasks 1 et 2 ; aucune redéfinition |
| `ImportPlan` | Plan import Task 4, `crates/rawzelia-app/src/lightroom_import.rs` | Tasks 1 et 2 ; aucune redéfinition |
| `LightroomBatchStore` | Plan import Task 5, `crates/rawzelia-storage/src/lightroom_batch.rs` | Task 2 ; aucune redéfinition |
| `ImportedArtifact` | Plan import Tasks 1 et 6, `artifacts.rs` | Task 2 ; aucune redéfinition |
| `ReferenceJpegSelection` | Plan core, `rawzelia-contract/src/commands.rs` | Task 2 la consomme ; `reference.rs` ne fait que valider et recalculer le fingerprint |
| `CatalogStore`, `LibraryQuery`, `RenderRequest`, `JobState`, `AppRuntime`, `AssetRecord`, `StorageError`, `EditDocument`, `RenderTarget`, `RenderedImage`, `RenderPoll`, `InteractiveRenderer`, `CacheKey`, `AssetId`, `JobId`, `AppError` | Plan core, fichiers canoniques des crates correspondantes | Tasks 3, 4 et 6 ; seules les méthodes prévues sont étendues |
| `LegacyCatalogAdapterV1`, `LegacyCatalogAdapterV2` | Hardening Task 1, `rawzelia-lightroom-import/src/adapters/legacy.rs` | Tasks 1, 2 et 9 |
| `FixtureManifest`, `CatalogFixture`, `FixtureCase` | Plan import Task 3, `rawzelia-lightroom-import/src/fixture_preflight.rs` ; hardening Task 1 ne fait que modifier/étendre ce modèle | Tasks 1, 2 et 9 |
| `SourceValidation`, `ImportBatchRequest` | Task 2, `rawzelia-lightroom-import/src/import_transaction.rs` | Task 2 et Task 9 |
| `VolumeId`, `VolumeStatus`, `VolumeError`, `LocationRefreshReport`, `LocationCatalog` | Task 3, `rawzelia-storage/src/volume_status.rs` | Tasks 3, 4 et 9 |
| `PerformanceScenario`, `PerformanceBudget`, `PerformanceBudgetReport`, `PerformanceError` | Task 4, `rawzelia-imaging/src/performance.rs` | Tasks 4 et 9 |
| `Keymap`, `KeyBinding`, `KeyStroke`, `FocusContext` | Plan core, `rawzelia-contract/src/keymap.rs` ; hardening Task 5 ne fait que modifier le fichier et consommer les types | Task 5 et rapport qualité Task 9 |
| `UiManifest`, `UiControl`, `MessageCatalog`, `I18nError`, `AccessibilityError` | Hardening Task 5, `rawzelia-desktop` | Task 5 et rapport qualité Task 9 |
| `BackupReport`, `RestoreReport`, `BackupError`, `RestoreError` | Plan core, `rawzelia-storage/src/backup.rs` ; hardening Task 6 ne fait que modifier/valider les types et méthodes existants | Tasks 6 et 9 |
| `DiagnosticsConfig`, `CrashContext`, `CrashConsent`, `DiagnosticBundle`, `DiagnosticError`, `CrashDiagnosticsMode` | Hardening Task 6, `rawzelia-app/src/diagnostics.rs` | Tasks 6 et 9 |
| `Version`, `CommitId`, `Artifact`, `ArtifactStage`, `BuildManifest`, `CargoMetadata`, `PackageLicense`, `SpdxReport`, `ReleaseManifest`, `ReleaseError`, `GateId`, `GateResult`, `GateFailure`, `PublicationCriteria`, `ValidationReport`, `PublicationDecision` | Task 7, `rawzelia-contract/src/release_contract.rs` | Tasks 7, 8 et 9 |

Une tâche suivante consomme le symbole du ledger et ne le redéclare pas. Les types canoniques des plans core/import restent propriétaires de ces plans ; toute extension locale porte un nom distinct et une justification dans la tâche propriétaire.

## Canonical Interfaces Reused Without Redefinition

- `LightroomAdapter` reste exactement `pub trait LightroomAdapter { fn schema_id(&self) -> LightroomSchemaId; fn can_read(&self, workspace: &WorkspaceCopy) -> Result<bool, ImportError>; fn dry_run(&self, workspace: &WorkspaceCopy) -> Result<ImportReport, ImportError>; fn emit_records(&self, workspace: &WorkspaceCopy, sink: &mut dyn ImportRecordSink) -> Result<(), ImportError>; }`.
- `LightroomBatchStore` reste exactement `pub trait LightroomBatchStore { fn begin_import(&mut self, source: &LightroomSourceIdentity, expected: &ImportCounts) -> Result<ImportBatchId, StorageError>; fn append_record(&mut self, batch_id: &ImportBatchId, record: &ImportedRecord) -> Result<(), StorageError>; fn append_artifact(&mut self, batch_id: &ImportBatchId, artifact: &ImportedArtifact) -> Result<(), StorageError>; fn verify_import(&self, batch_id: &ImportBatchId, expected: &ImportCounts, expected_source_ids: &[String]) -> Result<BatchVerification, StorageError>; fn commit_import(&mut self, batch_id: &ImportBatchId) -> Result<(), StorageError>; fn rollback_import(&mut self, batch_id: &ImportBatchId) -> Result<(), StorageError>; }`.
- `CatalogStore` reste exactement `pub trait CatalogStore: Send + Sync { fn open(path: &Path) -> Result<Self, StorageError> where Self: Sized; fn migrate(&self) -> Result<u32, StorageError>; fn upsert_asset(&self, asset: &AssetRecord) -> Result<(), StorageError>; fn find_asset(&self, asset_id: AssetId) -> Result<Option<AssetRecord>, StorageError>; fn remove_asset(&self, asset_id: AssetId) -> Result<(), StorageError>; fn page_library(&self, query: &LibraryQuery) -> Result<LibraryPage, StorageError>; fn append_revision(&self, asset_id: AssetId, document: &EditDocument) -> Result<RevisionId, StorageError>; fn load_edit_document(&self, asset_id: AssetId) -> Result<Option<EditDocument>, StorageError>; fn backup_to(&self, destination: &Path) -> Result<BackupReport, StorageError>; fn restore_from(source: &Path, destination: &Path) -> Result<RestoreReport, StorageError> where Self: Sized; }`.
- `LibraryQuery` reste `pub struct LibraryQuery { pub text: Option<String>, pub date_from: Option<String>, pub date_to: Option<String>, pub collection_id: Option<String>, pub rating: Option<Rating>, pub flag: Option<Flag>, pub label: Option<LabelColor>, pub keyword: Option<String>, pub page: u32, pub page_size: u32 }`.
- `RenderRequest` reste `pub struct RenderRequest { pub asset_id: AssetId, pub source: PathBuf, pub document: EditDocument, pub generation: u64, pub target: RenderTarget }`; `JobState` reste exactement `pub type JobState = JobEventState`.
- `ImportStatus` reste exactement `pub enum ImportStatus { Exact, Approximated, PreservedOnly, Unsupported, Ignored }` et `FieldProvenance` exactement `pub struct FieldProvenance { pub source_schema: LightroomSchemaId, pub source_id: String, pub field_path: String, pub artifact: String }`.
- `WorkspaceCopy` reste exactement `pub struct WorkspaceCopy { pub root: PathBuf, pub catalog_path: PathBuf, pub associated_root: Option<PathBuf>, pub source: LightroomSourceIdentity, pub workspace_fingerprint: Fingerprint }`.
- `ImportPlan` reste exactement `pub struct ImportPlan { pub report_id: ImportReportId, pub source: LightroomSourceIdentity, pub workspace: WorkspaceCopy, pub expected_counts: ImportCounts, pub expected_source_ids: Vec<String>, pub target_catalog: PathBuf, pub reference_jpeg: Option<ReferenceJpegSelection> }`.
- `StorageError` reste exactement `pub enum StorageError { Io { code: String, message_key: String }, Sql { code: String, message_key: String }, Conflict { fields: Vec<String> }, InvalidBackup { code: String, message_key: String }, Import { code: String, message_key: String, source_id: Option<String>, field_path: Option<String> } }`.
- `ImportError` reste exactement `pub struct ImportError { pub code: ImportErrorCode, pub message_key: String, pub source_id: Option<String>, pub field_path: Option<String> }`.
- `KeyStroke`, `KeyBinding`, `Keymap`, `FocusContext` et `dispatch_keystroke(keymap: &Keymap, stroke: KeyStroke, focus: FocusContext) -> Option<AppCommand>` restent les signatures canoniques du plan core ; le hardening ne les redéfinit pas.
- `BackupReport` reste exactement `pub struct BackupReport { pub catalog_sha256: [u8; 32], pub sidecar_sha256: BTreeMap<PathBuf, [u8; 32]>, pub entry_count: u64 }` et `RestoreReport` exactement `pub struct RestoreReport { pub restored_catalog: PathBuf, pub restored_sidecars: u64, pub verified: bool }`.

### Task 1: Register and Test Older Lightroom Catalog Adapters

**Files:**
- Modify: `crates/rawzelia-lightroom-import/src/adapter.rs`
- Create: `crates/rawzelia-lightroom-import/src/adapters/mod.rs`
- Create: `crates/rawzelia-lightroom-import/src/adapters/legacy.rs`
- Modify: `crates/rawzelia-lightroom-import/src/fixture_preflight.rs`
- Modify: `tests/fixtures/lightroom/manifest.toml`
- Create: `crates/rawzelia-lightroom-import/tests/legacy_adapters.rs`
- Modify: `crates/rawzelia-lightroom-import/src/lib.rs`

**Interfaces:**
- Consumes: `WorkspaceCopy`, `LightroomSchemaId`, `ImportStatus`, `FieldProvenance`, `ImportCounts`, `ImportReport`, `ImportRecordSink`, `ImportedRecord`, `ImportError` et le trait canonique `LightroomAdapter` du plan import. La sélection existante `select_adapter(workspace: &WorkspaceCopy) -> Result<Box<dyn LightroomAdapter>, ImportError>` est étendue, pas redéfinie sous un autre contrat.
- Produces:
  - `pub struct LegacyCatalogAdapterV1 { pub schema: LightroomSchemaId, pub fixture: CatalogFixture }` et `pub struct LegacyCatalogAdapterV2 { pub schema: LightroomSchemaId, pub fixture: CatalogFixture }`, implémentant chacun le trait canonique `LightroomAdapter` avec `fn schema_id(&self) -> LightroomSchemaId`, `fn can_read(&self, workspace: &WorkspaceCopy) -> Result<bool, ImportError>`, `fn dry_run(&self, workspace: &WorkspaceCopy) -> Result<ImportReport, ImportError>` et `fn emit_records(&self, workspace: &WorkspaceCopy, sink: &mut dyn ImportRecordSink) -> Result<(), ImportError>`.
  - `pub fn extend_fixture_manifest_for_legacy(manifest: &mut FixtureManifest, fixture_root: &Path) -> Result<(), ImportError>` ; cette fonction modifie le modèle canonique du plan import, ne redéfinit pas `FixtureManifest`, ajoute les entrées `legacy` et les cas manquants, et valide que tous les chemins restent sous l’unique `RAWZELIA_FIXTURES_DIR`.
  - Le `FixtureCase` canonique du plan import est seulement étendu avec `pub active_marker: Option<PathBuf>` et `pub incomplete_component: Option<PathBuf>`; `load_fixture_manifest` préflighte ces champs avant toute consommation, exige le marqueur actif présent, exige le composant incomplet absent et conserve `associated_root: None` pour `cases.missing-lrdata`.
  - `pub fn registered_adapters(manifest: &FixtureManifest) -> Result<Vec<Box<dyn LightroomAdapter>>, ImportError>` et l’extension de la signature canonique existante `pub fn select_adapter(workspace: &WorkspaceCopy) -> Result<Box<dyn LightroomAdapter>, ImportError>` ; la sélection charge le manifest canonique depuis l’unique `RAWZELIA_FIXTURES_DIR`, puis applique l’ordre strict `latest` puis `legacy.v1` puis `legacy.v2`, sans mise à niveau Adobe et sans schéma deviné.

- [ ] **Step 1: Write the failing tests for adapter ordering, manifest preflight, and fixture coverage**

  Add `fixture_manifest_preflight_requires_rawzelia_fixture_root`, `registered_adapters_are_ordered_latest_then_legacy_v1_then_legacy_v2`, `latest_schema_selects_latest_adapter`, `previous_schema_selects_legacy_v1_without_upgrade`, `older_schema_selects_legacy_v2_without_upgrade`, `unknown_schema_returns_unsupported`, `active_and_incomplete_cases_are_declared_before_consumption`, and `legacy_dry_run_keeps_source_read_only` to `legacy_adapters.rs`. The tests load all `schema_id`, `catalog_path`, `associated_root`, `expected_counts`, `expected_source_ids` and case paths from the canonical `FixtureManifest` in `tests/fixtures/lightroom/manifest.toml`; they set only `RAWZELIA_FIXTURES_DIR` to the fixture root, assert the selected `LightroomSchemaId`, assert that serialized `ImportStatus` is `preserved_only` or `unsupported` for unknown fields, and assert unchanged source hash and mtime before adapter consumption.

  Extend the canonical manifest with the same latest entry already defined by the import plan: `latest.schema_id = "lightroom-latest-corpus"`, `latest.catalog_path = "latest/catalog.lrcat"`, `latest.associated_root = "latest/catalog.lrdata"`, its expected counts/IDs and reference JPEG. Add only the legacy entries under that same root: `legacy.v1.schema_id = "lightroom-legacy-v1"`, `legacy.v1.catalog_path = "legacy/v1/catalog.lrcat"`, `legacy.v1.associated_root = "legacy/v1/catalog.lrdata"`, with matching `expected-counts.toml`, `source-ids.txt` and `reference/asset-0001.jpg`; repeat with `legacy/v2` and the corresponding `legacy/v2/...` paths. Extend canonical `FixtureCase` with `active_marker` and `incomplete_component`, then declare `cases.active` and `cases.incomplete` before any adapter consumes them. Keep `cases.missing-lrdata.catalog_path = "cases/missing-lrdata/catalog.lrcat"` with `associated_root` absent. All paths are relative to the one `RAWZELIA_FIXTURES_DIR`; no adapter may be registered without a manifest entry and fixture path.

- [ ] **Step 2: Run the focused test to verify it fails**

  Run from the repository root:

  ```bash
  test -n "${RAWZELIA_FIXTURES_DIR:-}" && cargo test -p rawzelia-lightroom-import --test legacy_adapters registered_adapters_are_ordered_latest_then_legacy_v1_then_legacy_v2 -- --exact
  ```

  Expected: `FAIL` with `fixture manifest preflight failed` or `no adapter registered for schema=lightroom-legacy-v1`; the failure must occur before any catalog write.

- [ ] **Step 3: Implement the minimal legacy adapter registry**

  Extend the canonical `fixture_preflight.rs` parser and implement `LegacyCatalogAdapterV1` and `LegacyCatalogAdapterV2` in the files above. Declare and preflight `latest`, `legacy.v1`, `legacy.v2`, `cases.active` and `cases.incomplete` before registering consumers; validate `active_marker` and `incomplete_component` explicitly. Register the latest adapter first, then V1, then V2; select by the manifest’s exact `LightroomSchemaId`, never by a guessed version or an Adobe upgrade. Map only fields established by each fixture. Preserve unknown plugin data and unsupported Develop nodes with the canonical `FieldProvenance` and `ImportStatus`. Reject schemas absent from the manifest before opening the source for write.

- [ ] **Step 4: Run the adapter test suite to verify it passes**

  Run:

  ```bash
  test -n "${RAWZELIA_FIXTURES_DIR:-}" && cargo test -p rawzelia-lightroom-import --test legacy_adapters -- --nocapture
  ```

  Expected: `PASS`; preflight validates every required path, adapter order is latest then V1 then V2, all declared fixtures select the expected `LightroomSchemaId`, unknown schema is reported as `unsupported`, and source hashes and mtimes remain unchanged.

- [ ] **Step 5: Commit the independently reviewable adapter change**

  ```bash
  git add crates/rawzelia-lightroom-import/src/adapter.rs crates/rawzelia-lightroom-import/src/adapters/mod.rs crates/rawzelia-lightroom-import/src/adapters/legacy.rs crates/rawzelia-lightroom-import/src/fixture_preflight.rs crates/rawzelia-lightroom-import/src/lib.rs crates/rawzelia-lightroom-import/tests/legacy_adapters.rs tests/fixtures/lightroom/manifest.toml
  git commit -m "feat: add legacy Lightroom catalog adapters"
  ```

### Task 2: Harden Import Validation, Corruption Handling, and Batch Rollback

**Files:**
- Create: `crates/rawzelia-lightroom-import/src/import_transaction.rs`
- Modify: `crates/rawzelia-lightroom-import/src/lib.rs`
- Modify: `crates/rawzelia-storage/src/lib.rs`
- Create: `crates/rawzelia-lightroom-import/tests/import_rollback.rs`
- Create: `tests/fixtures/lightroom/corruptions.toml`

**Interfaces:**
- Consumes: `WorkspaceCopy`, `SourceInspection`, `LightroomAdapter`, `ImportPlan`, `ImportedRecord`, `ImportedArtifact`, `LightroomSourceIdentity`, `ImportCounts`, `ImportStatus`, `FieldProvenance`, `ImportError`, `ImportBatchId`, `ImportSummary` et `LightroomBatchStore` du plan import, `ReferenceJpegSelection` depuis `rawzelia-contract/src/commands.rs` du plan core, ainsi que `StorageError` et l’API SQLite/WAL de `rawzelia-storage`.
- Produces:
  - `pub fn validate_catalog_copy(source: &Path) -> Result<SourceValidation, ImportError>`
  - `pub struct ImportBatchRequest { pub workspace: WorkspaceCopy, pub plan: ImportPlan, pub adapter: Box<dyn LightroomAdapter>, pub artifacts: Vec<ImportedArtifact>, pub dry_run: bool }`
  - `pub fn execute_import_batch(request: ImportBatchRequest, target: &mut dyn LightroomBatchStore) -> Result<ImportSummary, ImportError>`
  - `pub fn rollback_import_batch(target: &mut dyn LightroomBatchStore, batch_id: ImportBatchId) -> Result<(), ImportError>`
  - `pub enum SourceValidation { Ready, Locked, Active, Incomplete, Corrupt, Unsupported }`

- [ ] **Step 1: Write red tests for source refusal and isolated rollback**

  Add `import_rejects_locked_or_corrupt_source_and_rolls_back_batch`, `unknown_plugin_data_does_not_abort_known_rows`, `rollback_removes_only_imported_batch`, `rollback_preserves_existing_catalog_rows`, `reference_jpeg_artifact_is_removed_with_batch_id`, and `partial_batch_reports_failed_assets`. Inject failures after workspace copy, after metadata insertion and before commit. Assert no source file mutation, no orphan sidecar, unchanged pre-existing asset count, explicit `ImportStatus`, `ImportBatchId` and batch status, and a report containing failed asset IDs. Create two batches with distinct `ImportedArtifact` JPEGs; rollback only the selected `batch_id` and assert its JPEG artifact disappears while the other batch’s JPEG and all pre-existing rows remain.

  `tests/fixtures/lightroom/corruptions.toml` must define the exact cases `locked`, `active`, `incomplete`, `corrupt`, `missing_lrdata` and `missing_volume`, each with `relative_path`, `expected_validation`, `expected_status` and `must_leave_source_unchanged = true`. The paths resolve only below `RAWZELIA_FIXTURES_DIR` and must match the case paths declared by the manifest from Task 1.

- [ ] **Step 2: Run the corruption test to verify it fails**

  Run:

  ```bash
  test -n "${RAWZELIA_FIXTURES_DIR:-}" && cargo test -p rawzelia-lightroom-import --test import_rollback import_rejects_locked_or_corrupt_source_and_rolls_back_batch -- --exact
  ```

  Expected: `FAIL` with `assertion failed: source_validation == SourceValidation::Locked` or `rollback journal is not implemented`.

- [ ] **Step 3: Implement bounded validation and a batch journal**

  Detect `.lrcat.lock`, active markers represented by `SourceInspection`, missing required source components and truncated/corrupt fixture signatures before mutation. Write every target mutation through the exact canonical `LightroomBatchStore::begin_import`, `append_record`, `append_artifact`, `verify_import(..., expected_source_ids)`, `commit_import` and `rollback_import` methods under one `ImportBatchId`. Append every request artifact, including the typed JPEG produced by `associate_reference_jpeg`, to that same batch. On failure or explicit rollback, call `rollback_import(&batch_id)` so the JPEG and all other new artifacts are removed only for that batch. Record created assets, sidecars, collection links and provenance rows. On failure, execute inverse operations from the journal and leave the source untouched. Preserve unknown plugin records as `preserved_only` or `unsupported` with their canonical `FieldProvenance`.

- [ ] **Step 4: Run the complete import hardening suite to verify it passes**

  Run:

  ```bash
  test -n "${RAWZELIA_FIXTURES_DIR:-}" && cargo test -p rawzelia-lightroom-import --test import_rollback -- --nocapture
  ```

  Expected: `PASS`; locked, active, incomplete and corrupt sources are classified, known rows survive unknown plugin data, rollback removes only the new batch, and all source hashes and mtimes are unchanged.

- [ ] **Step 5: Commit the transaction and rollback change**

  ```bash
  git add crates/rawzelia-lightroom-import/src/import_transaction.rs crates/rawzelia-lightroom-import/src/lib.rs crates/rawzelia-storage/src/lib.rs crates/rawzelia-lightroom-import/tests/import_rollback.rs tests/fixtures/lightroom/corruptions.toml
  git commit -m "fix: harden Lightroom import rollback"
  ```

### Task 3: Validate Large Archives and Missing or Removable Volumes

**Files:**
- Create: `crates/rawzelia-storage/src/volume_status.rs`
- Modify: `crates/rawzelia-storage/src/lib.rs`
- Create: `crates/rawzelia-storage/tests/archive_scale.rs`
- Create: `crates/rawzelia-storage/tests/volume_status.rs`
- Create: `tests/fixtures/catalog/archive_manifest.toml`

**Interfaces:**
- Consumes: `CatalogStore`, `AssetId`, `LocationId`, `LibraryQuery`, `LibraryPage`, `AssetAvailability`, `StorageError` et la table `locations` existante.
- Produces:
  - `pub struct VolumeId(pub String)`, `pub enum VolumeError { UnreadablePath, UnknownVolume, PermissionDenied, Io(String) }` et `pub enum VolumeStatus { Present { volume_id: VolumeId, removable: bool }, Disconnected { volume_id: VolumeId, removable: bool }, Missing, AccessDenied }`.
  - `pub struct LocationRefreshReport { pub present: u64, pub disconnected: u64, pub missing: u64, pub access_denied: u64 }`.
  - `pub trait VolumeResolver { fn resolve(&self, path: &Path) -> Result<VolumeStatus, VolumeError>; }`
  - `pub trait LocationCatalog: CatalogStore { fn refresh_location_status(&mut self, resolver: &dyn VolumeResolver) -> Result<LocationRefreshReport, StorageError>; }`
  - `pub fn refresh_locations(catalog: &mut dyn LocationCatalog, resolver: &dyn VolumeResolver) -> Result<LocationRefreshReport, StorageError>`
  - `pub fn search_assets(catalog: &dyn CatalogStore, query: &LibraryQuery) -> Result<LibraryPage, StorageError>`

- [ ] **Step 1: Write red tests for 10k/50k catalogs and volume transitions**

  Add `search_10000_p95_is_under_200ms`, `search_50000_multi_root_p95_is_under_300ms`, `refresh_locations_preserves_assets_when_volume_disconnects`, `removable_volume_reconnect_restores_present_state`, `missing_root_is_visible_without_deleting_assets`, `unicode_path_and_access_denied_are_reported`, and `archive_scale_does_not_regenerate_previews`. Generate metadata-only fixtures from `archive_manifest.toml` so the scale test does not require redistributing user photos.

- [ ] **Step 2: Run the scale test to verify it fails**

  Run:

  ```bash
  cargo test -p rawzelia-storage --test archive_scale search_50000_multi_root_p95_is_under_300ms -- --exact --nocapture
  ```

  Expected: `FAIL` with `search p95 budget is not enforced` or `volume status is not represented`.

- [ ] **Step 3: Implement indexed search and explicit location states**

  Add the minimum SQLite indexes for root, normalized metadata, collection membership and searchable text. Keep asset rows when a root becomes `Disconnected`, `Missing` or `AccessDenied`; update location status and last observation only. Use `VolumeId` plus relative path for removable media and preserve the absolute path for diagnostics. Do not trigger RAW decoding or preview regeneration during search or volume refresh. Implement `LocationCatalog` on the concrete SQLite catalog already backing `CatalogStore`, without changing the public storage boundary.

- [ ] **Step 4: Run the archive and volume suite to verify it passes**

  Run:

  ```bash
  cargo test -p rawzelia-storage --test archive_scale -- --nocapture
  cargo test -p rawzelia-storage --test volume_status -- --nocapture
  ```

  Expected: `PASS`; 10 000 and 50 000 fixtures meet their p95 budgets, missing and removable volumes remain visible, reconnection restores state, Unicode paths and access denial are explicit, and no asset is deleted by refresh.

- [ ] **Step 5: Commit the archive and volume hardening**

  ```bash
  git add crates/rawzelia-storage/src/volume_status.rs crates/rawzelia-storage/src/lib.rs crates/rawzelia-storage/tests/archive_scale.rs crates/rawzelia-storage/tests/volume_status.rs tests/fixtures/catalog/archive_manifest.toml
  git commit -m "test: validate archive scale and volume states"
  ```

### Task 4: Enforce Performance and Memory Budgets on the M1 Reference Machine

**Files:**
- Create: `crates/rawzelia-imaging/src/performance.rs`
- Modify: `crates/rawzelia-imaging/src/lib.rs`
- Create: `crates/rawzelia-imaging/tests/performance_budgets.rs`
- Create: `crates/rawzelia-desktop/tests/ui_budgets.rs`
- Create: `tests/fixtures/performance/scenarios.toml`

**Interfaces:**
- Consumes: le `RenderRequest` canonique (`pub struct RenderRequest { pub asset_id: AssetId, pub source: PathBuf, pub document: EditDocument, pub generation: u64, pub target: RenderTarget }`), `RenderPoll`, `InteractiveRenderer`, `CacheKey`, `AssetId`, `JobState`, `RenderedImage` et `render_cpu` du socle imaging/app.
- Produces:
  - `pub struct PerformanceScenario { pub fixture_root: PathBuf, pub asset_count: u64, pub root_count: u32, pub cache_warm: bool, pub triage_duration_seconds: u64 }` et `pub enum PerformanceError { FixtureMissing, BudgetExceeded(String), UnsupportedPlatform, Measurement(String) }`.
  - `pub struct PerformanceBudget { pub catalog_open_hot_ms: u64, pub search_10k_p95_ms: u64, pub search_50k_p95_ms: u64, pub loupe_hot_p95_ms: u64, pub proxy_adjustment_p95_ms: u64, pub grid_min_fps: u32, pub pan_min_fps: u32, pub max_ui_block_ms: u64, pub max_triage_rss_bytes: u64 }`
  - `pub struct PerformanceBudgetReport { pub catalog_open_p95_ms: u64, pub search_10k_p95_ms: u64, pub search_50k_p95_ms: u64, pub loupe_p95_ms: u64, pub proxy_adjustment_p95_ms: u64, pub grid_fps: u32, pub pan_fps: u32, pub max_ui_block_ms: u64, pub memory_peak_bytes: u64, pub cpu_export: bool, pub stale_generation_count: u32, pub machine: String }`.
  - `pub const fn v01_m1_16gb() -> PerformanceBudget`
  - `pub fn run_performance_scenario(scenario: PerformanceScenario, budget: &PerformanceBudget) -> Result<PerformanceBudgetReport, PerformanceError>`
  - `pub fn resident_bytes() -> Result<u64, PerformanceError>`

- [ ] **Step 1: Write red tests for exact spec budgets and generation invalidation**

  Add `performance_budgets_m1_16gb_pass`, `proxy_adjustment_p95_is_under_100ms`, `loupe_hot_p95_is_under_400ms`, `grid_scroll_reaches_50_fps`, `pan_reaches_60_fps`, `ui_block_never_exceeds_500ms`, `triage_stays_under_1_5_gib_for_30_minutes`, `export_remains_cpu_only`, and `stale_generation_is_never_published`. Use the M1/16 GB scenario manifest and assert the exact values from `v01_m1_16gb()`.

- [ ] **Step 2: Run the budget test to verify it fails**

  Run:

  ```bash
  cargo test -p rawzelia-imaging --test performance_budgets performance_budgets_m1_16gb_pass -- --exact --nocapture
  ```

  Expected: `FAIL` with `performance budget report is not implemented` or a measured budget assertion such as `proxy_adjustment_p95_ms=... > 100`.

- [ ] **Step 3: Implement bounded measurement and budget assertions**

  Measure only the scenarios in the fixture manifest. Sample resident memory outside the profiler and keep the job queue bounded. Preserve generation IDs through proxy, tile and cache publication. Record CPU/GPU path, cache state, sample count and machine metadata in `PerformanceBudgetReport`. Do not optimize beyond the declared budgets and do not move final export to GPU.

- [ ] **Step 4: Run imaging and desktop budget tests to verify they pass**

  Run:

  ```bash
  cargo test -p rawzelia-imaging --test performance_budgets -- --nocapture
  cargo test -p rawzelia-desktop --test ui_budgets -- --nocapture
  ```

  Expected: `PASS` on the M1 reference machine; the report shows all p95, fps, UI block, memory, CPU export and stale-generation gates within the spec budgets.

- [ ] **Step 5: Commit the performance gates**

  ```bash
  git add crates/rawzelia-imaging/src/performance.rs crates/rawzelia-imaging/src/lib.rs crates/rawzelia-imaging/tests/performance_budgets.rs crates/rawzelia-desktop/tests/ui_budgets.rs tests/fixtures/performance/scenarios.toml
  git commit -m "perf: enforce v0.1 performance budgets"
  ```

### Task 5: Complete Keyboard, Accessibility, and Externalized Localization Contracts

**Files:**
- Modify: `crates/rawzelia-contract/src/keymap.rs`
- Modify: `crates/rawzelia-contract/src/lib.rs`
- Create: `crates/rawzelia-desktop/src/accessibility.rs`
- Create: `crates/rawzelia-desktop/src/i18n.rs`
- Create: `crates/rawzelia-desktop/locales/fr-FR.json`
- Create: `crates/rawzelia-desktop/locales/en-US.json`
- Create: `crates/rawzelia-desktop/tests/accessibility_i18n.rs`

**Interfaces:**
- Consumes: les signatures canoniques `KeyStroke { key: String, modifiers: Vec<String> }`, `KeyBinding { stroke: KeyStroke, command: AppCommand }`, `Keymap { version: u16, bindings: Vec<KeyBinding> }`, `FocusContext { Canvas, Panel, TextInput, Menu }` et `dispatch_keystroke(keymap: &Keymap, stroke: KeyStroke, focus: FocusContext) -> Option<AppCommand>` du plan core, ainsi que `AppCommand`, `AppRuntime`, `JobState` et le bridge Tauri du socle. Le fichier `crates/rawzelia-contract/src/keymap.rs` est modifié pour le comportement de hardening, mais reste le propriétaire unique de ces types et signatures.
- Produces:
  - `pub struct UiManifest { pub controls: Vec<UiControl> }`, `pub struct UiControl { pub id: String, pub role: String, pub label_key: String, pub tab_index: i32 }`, `pub struct MessageCatalog { pub locale: String, pub messages: BTreeMap<String, String> }`, `pub enum I18nError { MissingLocale, MissingKey(String), InvalidFile }` et `pub enum AccessibilityError { MissingRole(String), MissingName(String), InvalidFocusOrder(String) }`.
  - `pub fn load_messages(locale: &str) -> Result<MessageCatalog, I18nError>`
  - `pub fn validate_ui_manifest(manifest: &UiManifest, messages: &MessageCatalog) -> Result<(), AccessibilityError>`
  - `pub fn required_locales() -> &'static [&'static str]`

- [ ] **Step 1: Write red tests for keymap, focus, labels, focus order, and locale completeness**

  Add `keyboard_commands_respect_text_focus_and_locales_are_complete`, `all_required_controls_have_role_name_and_focus_order`, `menu_button_palette_and_shortcut_share_app_command`, `fr_locale_has_no_missing_required_keys`, `en_locale_is_structurally_ready`, and `tab_toggles_panels_without_losing_focus`. Assert `G`, `E`, `D`, `P`, `X`, `1` through `5` and `Tab`, with text input consuming printable strokes and the same `AppCommand` used by menu, button and shortcut.

- [ ] **Step 2: Run the accessibility test to verify it fails**

  Run:

  ```bash
  cargo test -p rawzelia-desktop --test accessibility_i18n keyboard_commands_respect_text_focus_and_locales_are_complete -- --exact
  ```

  Expected: `FAIL` with `missing locale key` or `interactive control has no accessible name`.

- [ ] **Step 3: Implement shared command dispatch and externalized messages**

  Move default key bindings into the versioned `Keymap`, dispatch through the existing `AppCommand` bus, and respect `FocusContext::TextInput`. Give every interactive control a stable role, accessible name, keyboard focus order and localized label through the minimal `UiManifest`; do not introduce a general accessibility framework or a new widget system. Keep all French strings in `fr-FR.json`; make `en-US.json` contain the same keys with English-ready values. Do not copy Adobe help, tooltips, onboarding or messages.

- [ ] **Step 4: Run the keyboard, accessibility, and locale suite to verify it passes**

  Run:

  ```bash
  cargo test -p rawzelia-desktop --test accessibility_i18n -- --nocapture
  cargo test -p rawzelia-contract --lib keymap -- --nocapture
  ```

  Expected: `PASS`; required commands dispatch correctly, text focus is respected, all controls have accessible metadata, French keys are complete, and English is structurally ready.

- [ ] **Step 5: Commit the UX hardening**

  ```bash
  git add crates/rawzelia-contract/src/keymap.rs crates/rawzelia-contract/src/lib.rs crates/rawzelia-desktop/src/accessibility.rs crates/rawzelia-desktop/src/i18n.rs crates/rawzelia-desktop/locales/fr-FR.json crates/rawzelia-desktop/locales/en-US.json crates/rawzelia-desktop/tests/accessibility_i18n.rs
  git commit -m "feat: harden keyboard accessibility and localization"
  ```

### Task 6: Implement Backup, Restore, Interrupted-Job Recovery, and Local Diagnostics Privacy

**Files:**
- Modify: `crates/rawzelia-storage/src/backup.rs`
- Create: `crates/rawzelia-app/src/diagnostics.rs`
- Modify: `crates/rawzelia-storage/src/lib.rs`
- Modify: `crates/rawzelia-app/src/lib.rs`
- Create: `crates/rawzelia-app/tests/recovery_privacy.rs`

**Interfaces:**
- Consumes: les signatures canoniques `BackupReport { catalog_sha256: [u8; 32], sidecar_sha256: BTreeMap<PathBuf, [u8; 32]>, entry_count: u64 }`, `RestoreReport { restored_catalog: PathBuf, restored_sidecars: u64, verified: bool }`, `BackupError { Io(String), HashMismatch, InconsistentCatalog, DestinationExists }`, `RestoreError { Io(String), InvalidManifest, HashMismatch, DestinationUntouched }`, `create_backup(catalog: &Path, sidecars: &[PathBuf], destination: &Path) -> Result<BackupReport, BackupError>` et `restore_backup(backup: &Path, destination: &Path) -> Result<RestoreReport, RestoreError>` du plan core. Consomme aussi `CatalogStore::backup_to(...) -> Result<BackupReport, StorageError>`, `CatalogStore::restore_from(...) -> Result<RestoreReport, StorageError>`, SQLite WAL catalog, `.rawzelia` paths, `JobState`, `JobId`, `AppRuntime::recover_interrupted_jobs(&self) -> Result<Vec<JobId>, AppError>` et l’abstraction filesystem locale existante.
- Produces:
  - `pub fn verify_backup_integrity(report: &BackupReport, destination: &Path) -> Result<(), BackupError>` et `pub fn verify_restore_integrity(report: &RestoreReport) -> Result<(), RestoreError>` dans `rawzelia-storage/src/backup.rs`; ces helpers ont des noms distincts et ne redéfinissent ni les rapports, ni les erreurs, ni `CatalogStore`.
  - `pub struct DiagnosticsConfig { pub telemetry_enabled: bool, pub crash_diagnostics: CrashDiagnosticsMode }`, `pub enum CrashDiagnosticsMode { Disabled, ConsentRequired }`, `pub struct CrashContext { pub version: String, pub platform: String, pub job_state: JobState, pub sanitized_logs: Vec<String> }`, `pub enum CrashConsent { Denied, Granted }`, `pub struct DiagnosticBundle { pub path: PathBuf, pub files: Vec<PathBuf> }` et `pub enum DiagnosticError { Io(String), ConsentRequired, NetworkTransportUnavailable }`.
  - `pub fn default_diagnostics_config() -> DiagnosticsConfig`
  - `pub fn collect_local_crash_diagnostics(context: &CrashContext, consent: CrashConsent) -> Result<Option<DiagnosticBundle>, DiagnosticError>`

- [ ] **Step 1: Write red tests for backup restoration, job recovery, and privacy defaults**

  Add `backup_restore_reopens_catalog_and_sidecars`, `restore_rejects_corrupt_backup_without_touching_destination`, `interrupted_jobs_become_resumable_or_explicitly_failed`, `thirty_interruptions_preserve_catalog_and_sidecars`, `diagnostics_default_to_no_telemetry`, `crash_bundle_requires_explicit_consent`, `consented_bundle_is_local_and_excludes_originals`, and `diagnostics_never_use_network_transport`.

- [ ] **Step 2: Run the privacy test to verify it fails**

  Run:

  ```bash
  cargo test -p rawzelia-app --test recovery_privacy diagnostics_default_to_no_telemetry -- --exact
  ```

  Expected: `FAIL` with `telemetry_enabled must default to false` or `crash diagnostics consent gate is missing`.

- [ ] **Step 3: Implement coherent backup and local-only diagnostics**

  Appeler `CatalogStore::backup_to` et `CatalogStore::restore_from` via le chemin WAL sûr, inclure les sidecars `.rawzelia` référencés et un manifeste de hashes, restaurer vers une nouvelle destination, puis valider la copie avec `verify_backup_integrity` et `verify_restore_integrity` avant de la déclarer utilisable. Réutiliser `create_backup` et `restore_backup` du core sans les redéfinir. Étendre uniquement l’implémentation de la méthode existante `AppRuntime::recover_interrupted_jobs` dans `rawzelia-app/src/lib.rs`; ne créer aucune fonction libre homonyme et conserver sa signature `Result<Vec<JobId>, AppError>`. Construire les diagnostics seulement après consentement explicite, inclure version, plateforme, état du job et logs nettoyés, exclure NEF, JPEG, payloads sidecar et contenu du catalogue, et n’exposer aucun émetteur réseau. Garder la télémétrie désactivée dans la configuration par défaut.

- [ ] **Step 4: Run recovery and privacy tests to verify they pass**

  Run:

  ```bash
  cargo test -p rawzelia-storage --lib backup -- --nocapture
  cargo test -p rawzelia-app --test recovery_privacy -- --nocapture
  ```

  Expected: `PASS`; backup restoration works on an independent copy, corrupt backups leave the destination unchanged, 30 interruptions preserve state, telemetry stays absent, and only a consented local diagnostic bundle can be created.

- [ ] **Step 5: Commit recovery and privacy hardening**

  ```bash
  git add crates/rawzelia-storage/src/backup.rs crates/rawzelia-storage/src/lib.rs crates/rawzelia-app/src/diagnostics.rs crates/rawzelia-app/src/lib.rs crates/rawzelia-app/tests/recovery_privacy.rs
  git commit -m "fix: harden recovery and local diagnostics privacy"
  ```

### Task 7: Enforce MPL-2.0 Community and Private Pro Separation

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/rawzelia-desktop/Cargo.toml`
- Modify: `crates/rawzelia-contract/src/capabilities.rs`
- Modify: `crates/rawzelia-contract/src/lib.rs`
- Create: `crates/rawzelia-contract/src/release_contract.rs`
- Create: `crates/rawzelia-app/src/pro_boundary.rs`
- Create: `crates/rawzelia-app/tests/community_boundary.rs`
- Create: `deny.toml`
- Create: `LICENSE`
- Create: `NOTICE`
- Create: `licenses/THIRD_PARTY.md`

**Interfaces:**
- Consumes: `Capability`, `CapabilitySet`, `community_capabilities()` et `pro_capabilities()` du socle, les workspace features, Cargo metadata et les contrats publics existants.
- Produces:
  - Dans `crates/rawzelia-contract/src/release_contract.rs`, les types uniques `pub struct Version(pub String)`, `pub struct CommitId(pub String)`, `pub enum ArtifactStage { Unsigned, Signed, Notarized, Stapled }`, `pub struct Artifact { pub path: PathBuf, pub stage: ArtifactStage, pub sha256: [u8; 32] }`, `pub struct BuildManifest { pub version: Version, pub commit: CommitId, pub feature_set: String, pub target: String, pub cargo_lock_sha256: [u8; 32] }`, `pub struct PackageLicense { pub name: String, pub version: String, pub license_spdx: String, pub source: String }`, `pub struct CargoMetadata { pub packages: Vec<PackageLicense> }`, `pub struct SpdxReport { pub packages: Vec<PackageLicense>, pub generated_from: CommitId }`, `pub enum LicenseError { MissingPackage(String), DisallowedLicense(String), InvalidSpdx(String) }`, `pub enum BoundaryError { PrivateDependency, ProCapability, InvalidFeatureSet }`, `pub struct ReleaseManifest { pub version: Version, pub commit: CommitId, pub toolchain: String, pub target: String, pub cargo_lock_sha256: [u8; 32], pub artifacts: Vec<Artifact> }`, `pub enum ReleaseError { MissingArtifact, HashMismatch, InvalidStage, Unsigned, NotarizationFailed }`, `pub enum GateId { Nef100, Archive10k, Archive50k, LegacyAdapters, ZeroSourceWrites, RecoveryCycles, PerformanceBudgets, BackupRestore, SignedNotarizedBundle, LicenseReport, AccessibilityQuality, I18nQuality, TelemetryDefault, LocalDiagnosticsConsent, PrivacyInvariant }`, `pub struct GateResult { pub passed: bool, pub command: String, pub environment: String, pub measured: Option<String>, pub budget: Option<String>, pub artifact_sha256: Option<[u8; 32]> }`, `pub struct GateFailure { pub gate: GateId, pub reason: String }`, `pub struct PublicationCriteria { pub require_100_nef_flow: bool, pub require_10k_archive: bool, pub require_50k_multi_root: bool, pub require_legacy_adapters: bool, pub require_zero_source_writes: bool, pub require_recovery_cycles: u32, pub require_performance_budgets: bool, pub require_backup_restore: bool, pub require_signed_notarized_bundle: bool, pub require_license_report: bool }`, `pub struct ValidationReport { pub gates: BTreeMap<GateId, GateResult>, pub artifacts: Vec<Artifact> }` et `pub enum PublicationDecision { Publish, Block { reasons: Vec<GateFailure> } }`.
  - `pub fn assert_community_manifest(manifest: &BuildManifest) -> Result<(), BoundaryError>`
  - `pub fn generate_spdx_report(metadata: &CargoMetadata) -> Result<SpdxReport, LicenseError>`
  - `pub fn community_build_requires_no_private_dependency() -> Result<(), BoundaryError>`

- [ ] **Step 1: Write red tests for capability isolation and license inventory**

  Add `community_build_has_no_pro_capabilities`, `community_build_does_not_require_private_repository`, `pro_fields_are_preserved_without_execution`, `spdx_manifest_lists_every_distributed_dependency`, `mpl_notice_is_present`, and `disallowed_license_fails_gate`.

- [ ] **Step 2: Run the Community boundary test to verify it fails**

  Run:

  ```bash
  cargo test -p rawzelia-app --test community_boundary community_build_has_no_pro_capabilities -- --exact
  ```

  Expected: `FAIL` with `Community manifest has Pro capability` or `private dependency is required`.

- [ ] **Step 3: Implement capability features and SPDX compliance**

  Define the feature owner exactly in `crates/rawzelia-desktop/Cargo.toml`: `[features]`, `default = ["community"]`, `community = []`; use the root `Cargo.toml` only for workspace package/license metadata, never for a workspace feature named `community`. Do not expose a public `pro` feature. Make the Community package compile and execute without the private repository. Preserve unknown `pro.*` sidecar fields without executing them. Add MPL-2.0 notices, dependency provenance, SPDX output and `cargo-deny` allow/deny rules for the actual dependency graph. Do not copy MPL-covered code into private Pro files.

- [ ] **Step 4: Run Community builds and license checks to verify they pass**

  Run:

  ```bash
  cargo test -p rawzelia-desktop --no-default-features --features community --locked
  cargo build -p rawzelia-desktop --no-default-features --features community --locked
  cargo test --workspace --locked
  cargo deny check licenses bans
  cargo test -p rawzelia-app --test community_boundary -- --nocapture
  ```

  Expected: `PASS`; Community compiles without private inputs, no Pro capability is required, every shipped dependency has an allowed documented license, MPL-2.0 notices are present, and Pro fields remain preserved-only when encountered.

- [ ] **Step 5: Commit the license and boundary gates**

  ```bash
  git add Cargo.toml crates/rawzelia-desktop/Cargo.toml crates/rawzelia-contract/src/capabilities.rs crates/rawzelia-contract/src/lib.rs crates/rawzelia-contract/src/release_contract.rs crates/rawzelia-app/src/pro_boundary.rs crates/rawzelia-app/tests/community_boundary.rs deny.toml LICENSE NOTICE licenses/THIRD_PARTY.md
  git commit -m "build: enforce Community and Pro boundaries"
  ```

### Task 8: Build, Sign, Notarize, and Reproduce the Community macOS Release

**Files:**
- Create: `scripts/release/macos-community.sh`
- Create: `scripts/release/verify-macos.sh`
- Create: `packaging/macos/Info.plist`
- Create: `packaging/macos/entitlements.plist`
- Create: `packaging/macos/notarize.sh`
- Create: `.github/workflows/release-macos-community.yml`
- Create: `release/macos-community-0.1.0.toml`
- Create: `crates/rawzelia-desktop/src/release_manifest.rs`
- Create: `crates/rawzelia-desktop/tests/release_manifest.rs`
- Create: `docs/release/macos-community.md`

**Interfaces:**
- Consumes: `Version`, `CommitId`, `Artifact`, `ArtifactStage`, `BuildManifest`, `ReleaseManifest`, `ReleaseError` et `community_build_requires_no_private_dependency()` depuis `crates/rawzelia-contract/src/release_contract.rs`, Cargo.lock, la feature Community du package `rawzelia-desktop`, l’environnement de signature Apple et le chemin du bundle.
- Produces:
  - `pub fn build_release_manifest(version: &Version, commit: &CommitId, artifacts: &[Artifact]) -> ReleaseManifest`
  - `pub fn verify_release_manifest(manifest: &ReleaseManifest, artifacts: &[Artifact]) -> Result<(), ReleaseError>`
  - `pub fn community_bundle_path(version: &Version) -> PathBuf`
  - shell entry point `scripts/release/macos-community.sh --version <version> --output <directory> [--dry-run]`; `--dry-run` ne signe ni ne notarie.
  - shell entry point `scripts/release/verify-macos.sh --dry-run <bundle> <release-manifest>` pour la vérification structurelle et `scripts/release/verify-macos.sh <bundle> <release-manifest>` pour la vérification stricte.
  - Contrat de sortie : `verify-macos.sh --dry-run` retourne `0` avec un état `unsigned`; la vérification stricte retourne `0` seulement si `codesign`, `spctl`, notarisation et stapling passent, `2` si le bundle est unsigned et `3` si la notarisation ou le stapling échoue.

- [ ] **Step 1: Write red tests for artifact manifests and Community-only bundles**

  Add `release_manifest_lists_unsigned_signed_and_notarized_artifacts`, `manifest_contains_source_revision_and_checksums`, `community_bundle_excludes_pro_capabilities`, `verify_rejects_changed_artifact`, and `dry_run_requires_no_apple_credentials`.

- [ ] **Step 2: Run the manifest test to verify it fails**

  Run:

  ```bash
  cargo test -p rawzelia-desktop --test release_manifest release_manifest_lists_unsigned_signed_and_notarized_artifacts -- --exact
  ```

  Expected: `FAIL` with `ReleaseManifest is not implemented` or `artifact stage unsigned is missing`.

- [ ] **Step 3: Implement a reproducible unsigned build followed by signing and notarization**

  Pin the Rust toolchain and Cargo.lock, set `SOURCE_DATE_EPOCH` from the source commit, build `rawzelia-desktop` with `--no-default-features --features community --locked` and without the private repository, and emit the canonical `ReleaseManifest` containing source revision, toolchain, target, dependency lock hash, unsigned bundle hash and signed artifact hash. Sign the Apple Silicon bundle with `codesign`, submit it with `xcrun notarytool`, staple with `xcrun stapler`, and verify with `codesign --verify` and `spctl`. Keep Apple credentials in CI secrets or the local environment. Compare unsigned bundle hashes for reproducibility; record `Unsigned`, `Signed`, `Notarized` and `Stapled` as separate `ArtifactStage` values because Apple’s ticket is external state.

- [ ] **Step 4: Run local release checks and the reproducibility dry run to verify they pass**

  Run:

  ```bash
  cargo test -p rawzelia-desktop --test release_manifest -- --nocapture
  ./scripts/release/macos-community.sh --version 0.1.0 --output target/release-check --dry-run
  ./scripts/release/verify-macos.sh --dry-run target/release-check/Rawzelia.app target/release-check/release-manifest.toml
  ```

  Expected: `PASS`; dry-run requires no Apple credentials, Community has no Pro payload, unsigned hashes are stable, manifest checksums match, and structural verification returns `0` with `stage=unsigned`.

  Run the strict verifier against the deliberately unsigned rehearsal and assert its negative contract:

  ```bash
  ./scripts/release/verify-macos.sh target/release-check/Rawzelia.app target/release-check/release-manifest.toml
  ```

  Expected: `FAIL` with exit code `2` and `stage=unsigned`; this is a required negative check, not a failed dry-run.

  For the credentialed release rehearsal on a macOS Apple Silicon runner, run:

  ```bash
  ./scripts/release/macos-community.sh --version 0.1.0 --output target/release-candidate
  codesign --verify --deep --strict --verbose=2 target/release-candidate/Rawzelia.app
  spctl --assess --type execute --verbose=4 target/release-candidate/Rawzelia.app
  xcrun stapler validate target/release-candidate/Rawzelia.app
  ```

  Expected: `PASS`; the bundle is signed, notarized, stapled, executable on Apple Silicon and the manifest records the exact artifacts.

- [ ] **Step 5: Commit the reproducible release procedure**

  ```bash
  git add scripts/release/macos-community.sh scripts/release/verify-macos.sh packaging/macos/Info.plist packaging/macos/entitlements.plist packaging/macos/notarize.sh .github/workflows/release-macos-community.yml release/macos-community-0.1.0.toml crates/rawzelia-desktop/src/release_manifest.rs crates/rawzelia-desktop/tests/release_manifest.rs docs/release/macos-community.md
  git commit -m "build: add reproducible signed macOS Community release"
  ```

### Task 9: Assemble the Test Matrix and Publication Gate

**Files:**
- Create: `tests/matrix/v0.1-publication.toml`
- Create: `crates/rawzelia-app/src/publication_gate.rs`
- Create: `crates/rawzelia-app/tests/publication_gate.rs`
- Create: `docs/release/v0.1-publication-gate.md`

**Interfaces:**
- Consumes: `GateId`, `GateResult`, `GateFailure`, `PublicationCriteria`, `ValidationReport`, `PublicationDecision` et `Artifact` depuis `crates/rawzelia-contract/src/release_contract.rs`, ainsi que les rapports des tests adapter, rollback, scale, performance, accessibilité, recovery, licence et packaging.
- Produces:
  - `pub fn evaluate_publication(report: &ValidationReport, criteria: &PublicationCriteria) -> PublicationDecision` dans `crates/rawzelia-app/src/publication_gate.rs`; les types de données restent ceux du ledger Task 7.

- [ ] **Step 1: Write the red publication-gate test and matrix**

  Add matrix rows for latest and older Lightroom fixtures, locked and corrupt catalogs, unknown plugin records, missing `.lrdata`, missing and removable volumes, 100 NEF end-to-end, 10 000-photo triage, 50 000-entry multi-root search, 20 reopen cycles, 30 interruption cycles, four reference lenses, ICC exports, accessibility quality, French strings and English-ready locale, Community build, SPDX, backup/restore and signed/notarized macOS bundle. Record telemetry default, local consent diagnostics and accessibility as informational quality rows; they are not required `PublicationCriteria` gates. Add `blocks_when_required_gate_fails`, `publishes_only_when_all_required_gates_pass`, `matrix_has_no_windows_or_pro_rows`, and `matrix_reports_informational_quality_rows`.

- [ ] **Step 2: Run the publication test to verify it fails**

  Run:

  ```bash
  cargo test -p rawzelia-app --test publication_gate blocks_when_required_gate_fails -- --exact
  ```

  Expected: `FAIL` with `PublicationDecision evaluator is not implemented` or `matrix is missing required gate: signed_notarized_bundle`.

- [ ] **Step 3: Implement the gate evaluator and release report**

  Require every blocking gate declared in `tests/matrix/v0.1-publication.toml`. Emit a human-readable report containing command, environment, fixture revision, measured value, budget, pass/fail status and artifact checksum. Block publication on source mutation, failed rollback, missing legacy fixture, failed performance or memory budget, missing backup/restore validation, missing license report, Community dependency leakage or unsigned/notarized bundle. Report telemetry default, consented local diagnostics, accessibility and i18n quality without making them publication blockers. Keep Windows and Pro absent from the v0.1 matrix.

- [ ] **Step 4: Run the full Community matrix and publication gate**

  Run:

  ```bash
  cargo test -p rawzelia-desktop --no-default-features --features community --locked --all-targets -- --nocapture
  cargo test --workspace --locked --all-targets -- --nocapture
  cargo deny check licenses bans
  cargo test -p rawzelia-app --test publication_gate -- --nocapture
  ./scripts/release/macos-community.sh --version 0.1.0 --output target/publication-rehearsal --dry-run
  ```

  Expected: `PASS`; the report contains every required blocking gate and informational quality row, no Windows or Pro row, all spec budgets and integrity criteria pass, the Community build has no private dependency, diagnostics default to local/off without becoming a blocker, and the release rehearsal is reproducible.

- [ ] **Step 5: Commit the publication matrix and gate**

  ```bash
  git add tests/matrix/v0.1-publication.toml crates/rawzelia-app/src/publication_gate.rs crates/rawzelia-app/tests/publication_gate.rs docs/release/v0.1-publication-gate.md
  git commit -m "test: add Rawzelia v0.1 publication gate"
  ```

## Definition of Done

- [ ] `ImportStatus`, `FieldProvenance`, `WorkspaceCopy`, `ImportError`, `LightroomSchemaId`, `LightroomAdapter`, `ImportPlan` et `LightroomBatchStore` restent les seuls contrats Lightroom utilisés ; aucun doublon de type ou de trait n’est introduit.
- [ ] `tests/fixtures/lightroom/manifest.toml` est préflighté via `RAWZELIA_FIXTURES_DIR`, référence exactement les fichiers latest, legacy V1, legacy V2 et cas limites, et porte comptages et IDs source attendus.
- [ ] Older Lightroom catalog adapters are registered after the latest adapter, each has a representative fixture, dry-run, field counts, corruption, lock and rollback tests, and no source mutation.
- [ ] Active, locked, incomplete and corrupt catalogs produce explicit classifications; unknown plugin data remains preserved or unsupported without aborting known imports.
- [ ] Import rollback removes only the new batch and preserves all pre-existing Rawzelia data; the reference JPEG artifact is removed by its exact `batch_id` and a prior batch’s JPEG remains intact.
- [ ] Missing, disconnected and removable volumes remain visible; reconnection restores state; Unicode paths and access denial are explicit.
- [ ] 10 000-photo and 50 000-entry multi-root tests pass their p95 search and opening budgets without preview regeneration.
- [ ] M1/16 GB performance tests pass the exact spec budgets for proxy, loupe, grid, pan, UI blocking, memory and CPU-only export.
- [ ] Keyboard commands, focus behavior, accessible names, focus order, French strings and English-ready keys pass automated quality tests through the minimal manifest; no disproportionate accessibility framework is added.
- [ ] Backup, restoration and 30 interruption cycles pass without loss; restored catalogs open on independent copies.
- [ ] Telemetry is absent by default, no network transport is available to diagnostics, and crash diagnostics require explicit local consent and exclude image/catalog contents.
- [ ] Community builds compile and run without the private repository or Pro capabilities; MPL-2.0 notices and SPDX dependency inventory are complete.
- [ ] macOS Apple Silicon Community bundle is reproducible before signing, signed, notarized, stapled and verified with `codesign`, `spctl` and `xcrun stapler`.
- [ ] Release procedure is documented, locked, repeatable and contains no secrets.
- [ ] Publication matrix blocks only on the declared v0.1 gates: 100 NEF flow, archive scale, Lightroom recovery, rollback, backup/restore, performance, licenses, Community isolation and strict packaging; accessibility, i18n, telemetry and local consent diagnostics are reported as quality/invariant rows.
- [ ] `verify-macos.sh --dry-run` accepts only the structural unsigned state, while strict verification fails unsigned artifacts and passes only signed, notarized and stapled artifacts.
- [ ] All release and publication types are owned by `crates/rawzelia-contract/src/release_contract.rs` and consumed by Tasks 7 to 9 without redeclaration.
- [ ] No Windows deliverable, Pro function or cloud dependency entered the v0.1 release.
- [ ] All task commits are reviewable and the working tree is clean before publication.
