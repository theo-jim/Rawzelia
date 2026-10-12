# Rawzelia Core v0.1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Construire en huit semaines le socle Community de Rawzelia v0.1 sur macOS Apple Silicon, avec un flux utilisable de 100 NEF Nikon D750, du catalogue à l’export, et les contrats stables pour Pro et Lightroom.

**Architecture:** Un workspace Rust de huit crates physiques porte un modular monolith local. Tauri 2 fournit le shell et la WebView. Les commandes et événements sont typés dans `rawzelia-contract`, sans REST. `rawzelia-contract` est la couche de schémas et de types partagés ; `rawzelia-domain` peut en dépendre, mais le contrat ne dépend jamais du domaine. SQLite porte l’index et l’état des jobs. Les sidecars `.rawzelia` portent les documents d’édition complets. Le CPU est l’oracle de rendu et d’export ; `wgpu` accélère seulement le proxy interactif, avec repli CPU.

**Tech Stack:** Rust stable épinglé par `rust-toolchain.toml`, Cargo workspace, Tauri major 2, TypeScript sans framework UI, SQLite en WAL, LibRaw derrière `rawzelia-imaging`, Little CMS pour ICC, `wgpu` pour le proxy GPU, `egui`/`eframe` optionnels derrière la feature `egui-shell`, tests Cargo, tests TypeScript, golden images et CI macOS Apple Silicon. Les versions exactes des dépendances sont celles validées au bootstrap et verrouillées dans `Cargo.lock`; aucune version future n’est inventée.

**Spec:** `docs/superpowers/specs/2026-10-12-rawzelia-v0.1-design.md`

## Global Constraints

- Cible v0.1 : macOS Apple Silicon, machine de référence Apple Silicon M1, 16 Go de RAM, SSD local, écran externe non calibré.
- Le flux de référence porte sur 100 NEF Nikon D750, avec 10 000 photos réelles et un test d’échelle à 50 000 entrées sur plusieurs racines.
- Le modèle de destination par défaut est `Photos/AAAA/AAAA-MM-JJ Nom-session/`; il reste configurable.
- Le catalogue local est SQLite en WAL, mono-utilisateur, hors ligne, avec une seule couche d’écriture.
- Les huit seules crates physiques sont `rawzelia-domain`, `rawzelia-app`, `rawzelia-storage`, `rawzelia-imaging`, `rawzelia-io`, `rawzelia-lightroom-import`, `rawzelia-contract` et `rawzelia-desktop`.
- `rawzelia-domain` ne dépend ni de l’UI, ni de SQL, ni du filesystem, ni de FFI.
- La direction des dépendances interdit tout cycle : `rawzelia-contract` porte les identifiants et DTO partagés, `rawzelia-domain` les réutilise, et `rawzelia-contract` ne dépend jamais de `rawzelia-domain`.
- Tauri 2 est le shell v0.1 ; `rawzelia-desktop/src/bin/egui_shell.rs` est le shell de repli derrière `egui-shell` si le gate UX Tauri des semaines 6 à 8 échoue.
- Les commandes et événements métier sont typés, versionnés et transportés par Tauri ; aucune API REST interne n’est créée.
- La Community compile, teste et exécute le flux local sans dépôt privé, réseau, compte ou module Pro.
- Les espaces `.rawzelia` sont `community.*`, `lightroom.*` et `pro.*`; les champs inconnus sont conservés sans exécution.
- Les statuts Lightroom sont exactement `exact`, `approximated`, `preserved_only`, `unsupported` et `ignored` dans la sérialisation.
- L’import Lightroom v0.1 lit une copie, une sauvegarde ou un export ; il refuse une source active ou verrouillée par `.lrcat.lock` et n’écrit jamais dans `.lrcat`, `.lrdata`, XMP ou les originaux.
- Le CPU est la référence pour le décodage, la colorimétrie, le développement et l’export ; le GPU ne sert qu’au proxy interactif et possède un fallback CPU.
- Les réglages sont non destructifs, versionnés par révision immuable, et les NEF restent en lecture seule.
- Les sidecars et exports utilisent fichier temporaire, flush, synchronisation, renommage atomique et mise à jour transactionnelle du catalogue.
- L’export couvre JPEG pleine résolution, JPEG web et PNG ; sRGB est le profil par défaut et un profil ICC choisi est enregistré et incorporé lorsque le format le permet.
- La Community est MPL-2.0 ; les modules Pro sont privés, séparés et optionnels à la compilation et à l’exécution.
- Les interfaces, identifiants, noms de fonctions, noms de fichiers et commentaires de code sont en anglais ; les chaînes UI v0.1 sont en français et externalisées.
- `RAWZELIA_FIXTURES_DIR` est l’unique racine de fixtures. Le manifest NEF suivi est `tests/fixtures/nef/manifest.toml` et résout le corpus externe sous `$RAWZELIA_FIXTURES_DIR/nef/`; le manifest Lightroom canonique est `tests/fixtures/lightroom/manifest.toml` et résout ses catalogues sous `$RAWZELIA_FIXTURES_DIR/lightroom/`. Aucune commande du core ne lit l’ancienne variable sans suffixe `_DIR` ni une seconde racine.
- Les ressources Adobe ne sont pas copiées ; les icônes, composants, textes, messages, presets et ressources Rawzelia sont propres.
- Les raccourcis par défaut sont `G`, `E`, `D`, `P`, `X`, `1` à `5` et `Tab`, avec keymap versionnée et comportement dépendant du focus.
- Le jalon fonctionnel de la semaine 8 couvre import, tri, développement essentiel, suppression, sidecar et export ; le parseur Lightroom complet, l’association d’un JPEG de référence Lightroom et les adaptateurs anciens continuent après ce gate dans le plan import ou le hardening.
- Les budgets du jalon sont p95 < 5 s pour l’ouverture à chaud du catalogue 10 000 photos, p95 < 200 ms pour la recherche 10 000 photos, p95 < 400 ms pour la loupe cache chaud, p95 < 100 ms pour un réglage visible sur proxy, 60 fps pour zoom/pan cache chaud et moins de 1,5 Go pendant 30 minutes de tri.
- Cette demande de rédaction n’exécute aucun commit et ne modifie aucun fichier produit. L’exécution future du plan crée et modifie uniquement les chemins listés dans ses tâches ; chaque tâche garde son commit de livraison.
- Le core crée seulement le squelette Cargo de `rawzelia-lightroom-import` et un garde source minimal. Le trait `LightroomAdapter`, `WorkspaceCopy`, `ImportError`, le dry-run, les adaptateurs, les artefacts et l’association optionnelle d’un JPEG Lightroom appartiennent au plan import.
- `rawzelia-contract/src/lightroom_import.rs` est le module canonique partagé pour `LightroomSchemaId`, `ImportStatus`, `FieldProvenance`, `ImportedField<T>` et `UnknownField`; `rawzelia-contract/src/lightroom.rs` ne définit aucun de ces types. `FieldProvenance` contient `source_schema: LightroomSchemaId`, `source_id`, `field_path` et `artifact`.
- `rawzelia-contract/src/commands.rs` est l’unique propriétaire de `AppCommand`, `InspectLightroomCommand`, `ExecuteLightroomCommand`, `ReferenceJpegSelection`, `RollbackLightroomCommand` et des commandes Lightroom partagées. `rawzelia-app` orchestre ces types et ne redéfinit jamais `AppCommand`; le plan import consomme `ReferenceJpegSelection` depuis ce module, tandis que `reference.rs` ne contient que les fonctions de sélection/association et aucun type homonyme.
- `serde_json::Value` n’apparaît que dans `UnknownField.raw_value`; les champs connus sont des sous-types Rust et des `ImportedField<T>`.
- `validate_source_guard` dans `crates/rawzelia-lightroom-import/src/source_guard.rs` est l’unique garde de refus. Le plan import le consomme depuis `inspect_source` et mappe explicitement `SourceGuardError` vers `ImportError` : `Missing -> SourceMissing`, `Active -> SourceActive`, `Locked -> SourceLocked`, `Incomplete -> SourceCorrupt` et `Io -> StorageFailure`.
- `rawzelia-io` réserve `IngestPlan`, `plan_ingest` et `execute_ingest` à la copie SD. `ImportPlan` appartient exclusivement au flux Lightroom dans `rawzelia-app`.
- `StorageError` est l’unique enum de stockage du core et reste consommé par le store Lightroom et le hardening; aucun plan ultérieur ne redéfinit une struct homonyme.
- `Keymap`, `BackupReport` et `RestoreReport` ont les signatures détaillées du hardening dès le core; le hardening les modifie et les étend sans les recréer ni redéfinir.
- Le crate desktop expose `default = ["community"]`, `community = []` et `egui-shell = ["dep:egui", "dep:eframe"]`; le binaire conditionnel reste dans ce crate et n’ajoute aucune crate physique.
- Le gate Tauri des semaines 6 à 8 comprend un spike, un smoke test borné et une décision explicite. En cas d’échec, le shell Tauri est gelé puis le binaire conditionnel `egui_shell` est produit dans le même crate desktop avec les mêmes `AppCommand`/`AppEvent`; deux UIs ne sont jamais développées en parallèle.

## Review Focus

- Source Lightroom active, verrouillée ou incomplète : `crates/rawzelia-lightroom-import/tests/source_guard.rs::locked_catalog_is_rejected_without_mutation` doit prouver le refus avant toute lecture métier.
- Copie SD interrompue, doublon ou hash divergent : `crates/rawzelia-io/tests/ingest.rs::duplicate_ingest_is_idempotent_and_interruption_is_resumable` doit prouver qu’un original non vérifié n’est jamais déclaré importé.
- Conflit catalogue, sidecar et XMP : `crates/rawzelia-storage/tests/reconciliation.rs::divergent_sidecar_requires_explicit_resolution` doit prouver l’absence d’écrasement silencieux.
- NEF corrompu, objectif non identifié, profil DCP absent ou Samyang sans EXIF : `crates/rawzelia-imaging/tests/lens_profiles.rs::samyang_manual_selection_works_without_exif` doit conserver une sélection explicite, les droits de profil et une correction manuelle déterministe.
- Résultat GPU obsolète ou backend indisponible : `crates/rawzelia-imaging/tests/render_generation.rs::stale_generation_is_not_published_and_cpu_fallback_is_available` doit empêcher un ancien rendu de remplacer un réglage récent.

## File Map

Les tâches suivantes créent les fichiers de production et de test. Aucun autre crate Rust physique ne doit apparaître dans le workspace.

- `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo/config.toml` : workspace, toolchain et cible Apple Silicon.
- `crates/rawzelia-domain/` : IDs, métadonnées normalisées, édition, révisions et statuts sans I/O.
- `crates/rawzelia-contract/` : commandes, événements, capacités, ressources, contrats Lightroom, keymap et layout versionnés.
- `crates/rawzelia-storage/` : SQLite, migrations, sauvegarde, sidecars, XMP et cache index.
- `crates/rawzelia-io/` : import vérifié, volumes, hash, modèle de chemin, export et impression abstraite.
- `crates/rawzelia-imaging/` : LibRaw, D750, CPU oracle, ICC, cache d’images, proxy GPU et fallback.
- `crates/rawzelia-app/` : cas d’usage, orchestration, jobs persistants et dispatch des contrats.
- `crates/rawzelia-lightroom-import/` : squelette Cargo et garde source minimal ; les contrats d’adaptateurs, le dry-run, la provenance de champ, les artefacts, les adaptateurs et le JPEG de référence restent dans le plan import.
- `crates/rawzelia-desktop/` : Tauri 2, commandes, événements, permissions, ressources locales, UI TypeScript et binaire optionnel `egui_shell` derrière `egui-shell`.
- `.github/workflows/ci.yml` : gates Cargo, UI, build Tauri et test du workspace.
- `tests/fixtures/nef/manifest.toml` : manifeste NEF suivi, avec corpus externe résolu sous `$RAWZELIA_FIXTURES_DIR/nef/`, golden, objectifs et comptages obligatoires.
- `tests/fixtures/lightroom/manifest.toml` : manifeste Lightroom canonique suivi par le plan import, résolu sous `$RAWZELIA_FIXTURES_DIR/lightroom/` et consommé sans seconde racine par les tests de compatibilité.

## Ownership ledger

| Contrat ou fichier | Propriétaire | Consommateurs autorisés |
|---|---|---|
| IDs, `Fingerprint`, `Adjustment`, formats et DTO partagés | Task 2, `rawzelia-contract/src/types.rs` | domaine, app, storage, io, imaging, desktop ; le domaine ne les redéfinit pas |
| `LightroomSchemaId`, `ImportStatus`, `FieldProvenance`, `ImportedField<T>`, `UnknownField` | Task 2, `rawzelia-contract/src/lightroom_import.rs` | domaine, plan import, storage et hardening ; le plan import passe ce chemin en `Modify`/extension, aucun homonyme dans `lightroom.rs` |
| `AppCommand`, `InspectLightroomCommand`, `ExecuteLightroomCommand`, `ReferenceJpegSelection`, `RollbackLightroomCommand` | Task 2, `rawzelia-contract/src/commands.rs` | app, desktop, plan import et bridge ; app orchestre seulement, aucune redéfinition de `AppCommand` ni de `ReferenceJpegSelection` |
| `AssetRecord`, `AssetMetadata`, `CaptureDate` | Task 2, `rawzelia-domain/src/metadata.rs` | storage, io, imaging |
| `crates/rawzelia-lightroom-import/src/lib.rs` | Task 1, squelette uniquement | Task 2 ajoute le garde minimal ; plan import ajoute ses modules métier |
| `crates/rawzelia-lightroom-import/src/source_guard.rs` et `validate_source_guard` | Task 2, garde minimal unique | `inspect_source` du plan import consomme et mappe `SourceGuardError` vers `ImportError` |
| `crates/rawzelia-contract/src/keymap.rs` | Task 2, `KeyStroke`/`KeyBinding`/`Keymap`/`FocusContext`/`dispatch_keystroke` | Tasks 8-9 et hardening modifient et étendent, jamais ne recréent |
| `crates/rawzelia-storage/src/backup.rs` | Task 3, `BackupReport`/`RestoreReport`/`BackupError`/`RestoreError` | hardening modifie et étend, jamais ne redéfinit |
| `StorageError` | Task 3, `rawzelia-storage/src/catalog.rs` et `src/lib.rs` | CatalogStore, LightroomBatchStore et hardening ; unique enum de stockage |
| `IngestPlan`, `plan_ingest`, `execute_ingest` | Task 4, `rawzelia-io/src/ingest.rs` | app et gate semaine 8 ; `ImportPlan` est réservé au plan Lightroom |
| `AppRuntime::recover_interrupted_jobs` | Task 7 | hardening étend cette méthode, aucune fonction libre homonyme |
| `crates/rawzelia-io/src/trash.rs` et `TrashBackend` | Task 7 | app et desktop via le contrat de suppression |
| `crates/rawzelia-desktop/src/bin/egui_shell.rs` et `egui-shell` | Task 8, binaire conditionnel du crate desktop | uniquement si le gate Tauri échoue ; même `AppCommand`/`AppEvent`, aucune neuvième crate |

Le plan import reste propriétaire de `source.rs`, `workspace.rs`, `errors.rs`, `adapter.rs`, `schema.rs`, `mapping.rs`, `artifacts.rs`, `develop.rs` et de ses types Lightroom détaillés. Le core ne crée aucun de ces chemins.

## Eight-week execution order

| Semaine | Travail principal | Gate de sortie |
|---|---|---|
| 1 | Task 1 et début Task 2 | Workspace à huit crates, contrat de capacités, IDs et namespaces sérialisables |
| 2 | Fin Task 2 et début Task 3 | Contrats Community/Pro/Lightroom, source verrouillée refusée, première migration SQLite |
| 3 | Fin Task 3 et Task 4 | Catalogue récupérable, sidecar atomique, import SD vérifié et idempotent |
| 4 | Fin Task 4 et début Task 5 | 100 fichiers NEF fixture analysables, aucun original déclaré avant hash vérifié |
| 5 | Fin Task 5, Task 6 et début Task 7 | Rendu CPU golden, proxy cache, fallback CPU, premier dispatch métier |
| 6 | Fin Task 7 et spike Tauri Task 8 | Bridge local, focus clavier et chargement d’assets mesurés ; décision de poursuivre Tauri |
| 7 | Task 9 et Task 10 | Développement, presets, copy/sync, export JPEG/PNG et impression simple |
| 8 | Task 11 et smoke Tauri | Gate complet sur 100 NEF, budgets, CI et build Apple Silicon ; si Tauri échoue, binaire `egui_shell` conditionnel dans `rawzelia-desktop` avec smoke de navigation/rendu |

Le parseur Lightroom complet, les adaptateurs de schémas anciens, Smart Previews, l’association explicite d’un JPEG Lightroom et la récupération détaillée de Develop restent dans le plan import postérieur au gate. Le core prépare uniquement le garde source et les namespaces canoniques.

### Task 1: Bootstrap du workspace et contrat de capacités

**Files:**
- Create: `Cargo.toml`
- Create: `Cargo.lock`
- Create: `rust-toolchain.toml`
- Create: `.cargo/config.toml`
- Create: `crates/rawzelia-domain/Cargo.toml`
- Create: `crates/rawzelia-app/Cargo.toml`
- Create: `crates/rawzelia-storage/Cargo.toml`
- Create: `crates/rawzelia-imaging/Cargo.toml`
- Create: `crates/rawzelia-io/Cargo.toml`
- Create: `crates/rawzelia-lightroom-import/Cargo.toml`
- Create: `crates/rawzelia-contract/Cargo.toml`
- Create: `crates/rawzelia-desktop/Cargo.toml`
- Create: `crates/rawzelia-contract/src/lib.rs`
- Create: `crates/rawzelia-contract/src/capabilities.rs`
- Create: `crates/rawzelia-contract/tests/capabilities.rs`
- Create: `crates/rawzelia-domain/src/lib.rs`
- Create: `crates/rawzelia-app/src/lib.rs`
- Create: `crates/rawzelia-storage/src/lib.rs`
- Create: `crates/rawzelia-imaging/src/lib.rs`
- Create: `crates/rawzelia-io/src/lib.rs`
- Create: `crates/rawzelia-lightroom-import/src/lib.rs`
- Create: `crates/rawzelia-desktop/src/lib.rs`
- Create: `crates/rawzelia-desktop/src/main.rs`

**Interfaces:**
- Consumes: aucune interface précédente.
- Produces: un workspace compilable avec huit manifests, un module bibliothèque minimal par crate, un binaire desktop minimal et un test cargo cible ; `pub const CONTRACT_VERSION: u16 = 1;`, `pub enum Capability { Community, Pro, LightroomReadOnly }`, `pub struct CapabilitySet { pub contract_version: u16, pub capabilities: Vec<Capability> }`, `pub fn community_capabilities() -> CapabilitySet` et `pub fn pro_capabilities() -> CapabilitySet` dans `rawzelia-contract`.

- [ ] **Step 1: Bootstrap the empty workspace and write the failing test**

  Créer d’abord le workspace, les huit `Cargo.toml`, `rust-toolchain.toml`, `.cargo/config.toml`, `Cargo.lock`, les `src/lib.rs` vides et `crates/rawzelia-desktop/src/main.rs` minimal. Créer ensuite `crates/rawzelia-contract/src/capabilities.rs` et le test `community_capabilities_are_offline_and_versioned` dans `crates/rawzelia-contract/tests/capabilities.rs`. Le test doit sérialiser `community_capabilities()`, vérifier `contract_version == 1`, contenir `Community`, exclure `Pro`, et accepter `LightroomReadOnly`. Ne définir aucune fonction de production à ce stade.

- [ ] **Step 2: Run test to verify it fails**

  Run from repository root: `cargo test -p rawzelia-contract --test capabilities -- --exact community_capabilities_are_offline_and_versioned`

  Expected: FAIL with `error[E0432]: unresolved import rawzelia_contract::community_capabilities` or the equivalent missing-symbol error.

- [ ] **Step 3: Implement the minimal capability contract**

  Implémenter uniquement les types et fonctions de capacité dans `rawzelia-contract/src/capabilities.rs`, exporter le module depuis `src/lib.rs`, épingler le toolchain approuvé et configurer la cible `aarch64-apple-darwin`. Les huit crates restent sans logique produit. Ajouter seulement les dépendances nécessaires au contrat et conserver `Cargo.lock` généré par le bootstrap.

- [ ] **Step 4: Run test to verify it passes**

  Run: `cargo test -p rawzelia-contract --test capabilities -- --exact community_capabilities_are_offline_and_versioned`

  Expected: PASS.

- [ ] **Step 5: Commit**

  ```bash
  git add Cargo.toml Cargo.lock rust-toolchain.toml .cargo/config.toml crates
  git commit -m "chore: bootstrap rawzelia workspace contracts"
  ```

### Task 2: Modèle domaine, contrats canoniques et garde source minimal

**Files:**
- Create: `crates/rawzelia-domain/src/ids.rs`
- Create: `crates/rawzelia-domain/src/metadata.rs`
- Create: `crates/rawzelia-domain/src/edit.rs`
- Create: `crates/rawzelia-domain/src/lightroom.rs`
- Modify: `crates/rawzelia-domain/src/lib.rs`
- Modify: `crates/rawzelia-domain/Cargo.toml`
- Modify: `crates/rawzelia-app/Cargo.toml`
- Modify: `crates/rawzelia-contract/src/lib.rs`
- Modify: `crates/rawzelia-contract/Cargo.toml`
- Create: `crates/rawzelia-contract/src/types.rs`
- Create: `crates/rawzelia-contract/src/lightroom_import.rs`
- Create: `crates/rawzelia-contract/src/commands.rs`
- Create: `crates/rawzelia-contract/src/events.rs`
- Create: `crates/rawzelia-contract/src/keymap.rs`
- Create: `crates/rawzelia-contract/tests/serialization.rs`
- Create: `crates/rawzelia-contract/tests/lightroom_import_contract.rs`
- Create: `crates/rawzelia-contract/tests/lightroom_commands.rs`
- Create: `crates/rawzelia-domain/tests/serialization.rs`
- Create: `crates/rawzelia-app/tests/lightroom_contract.rs`
- Create: `crates/rawzelia-app/tests/lightroom_command_contract.rs`
- Modify: `crates/rawzelia-lightroom-import/src/lib.rs`
- Create: `crates/rawzelia-lightroom-import/src/source_guard.rs`
- Create: `crates/rawzelia-lightroom-import/tests/source_guard.rs`

**Interfaces:**
- Consumes: `CONTRACT_VERSION`, `CapabilitySet` et les huit crates du Task 1.
- Produces:
  - Dans `rawzelia-contract/src/types.rs` : `pub struct AssetId(pub Uuid);`, `pub struct LocationId(pub Uuid);`, `pub struct RevisionId(pub u64);`, `pub struct JobId(pub Uuid);`, `pub struct PresetId(pub Uuid);`, `pub struct RequestId(pub Uuid);`, `pub struct Fingerprint { pub size: u64, pub sha256: [u8; 32] }`, `pub struct ResourceRef { pub id: String, pub mime: String, pub width: u32, pub height: u32 }`;
  - Dans `rawzelia-contract/src/types.rs` : `pub enum Rating { None, One, Two, Three, Four, Five }`, `pub enum Flag { None, Pick, Reject }`, `pub enum LabelColor { None, Red, Yellow, Green, Blue, Purple }`, `pub enum Adjustment { Exposure { value: f32 }, Contrast { value: f32 }, Highlights { value: f32 }, Shadows { value: f32 }, WhiteBalance { temperature: i32, tint: i32 }, Crop { x: f32, y: f32, width: f32, height: f32 }, Rotation { degrees: f32 }, Straighten { degrees: f32 }, LensCorrection { profile: String, amount: f32 } }`, `pub enum SettingSection { Tone, WhiteBalance, Geometry, LensCorrection, All }`, `pub enum PreviewTarget { Grid, Loupe, OneToOne }`, `pub enum ExportFormat { JpegFull, JpegWeb, Png }`, `pub enum ExportProfile { Srgb, Icc { path: PathBuf } }`;
  - Dans `rawzelia-domain/src/metadata.rs` : `pub struct CaptureDate { pub year: i32, pub month: u8, pub day: u8, pub hour: u8, pub minute: u8, pub second: u8 }`;
  - `pub struct AssetMetadata { pub width: u32, pub height: u32, pub orientation: u16, pub camera_model: Option<String>, pub lens_model: Option<String>, pub keywords: Vec<String> }`;
  - `pub struct AssetRecord { pub asset_id: AssetId, pub location_id: LocationId, pub original_path: PathBuf, pub relative_path: String, pub captured_at: Option<CaptureDate>, pub metadata: AssetMetadata, pub fingerprint: Fingerprint, pub availability: AssetAvailability, pub rating: Rating, pub flag: Flag, pub label: LabelColor }` dans `rawzelia-domain`; `rawzelia-domain/src/ids.rs` réexporte les types partagés de `rawzelia-contract` sans les redéfinir;
  - `pub enum AssetAvailability { Present, Missing, Moved, VolumeOffline, PermissionDenied }`;
  - Dans `rawzelia-contract/src/lightroom_import.rs` : `pub struct LightroomSchemaId(pub String);`, `pub enum ImportStatus { Exact, Approximated, PreservedOnly, Unsupported, Ignored }` sérialisé en `exact`, `approximated`, `preserved_only`, `unsupported`, `ignored`; ce module est le propriétaire canonique consommé par le plan import et le hardening;
  - Dans `rawzelia-contract/src/lightroom_import.rs` : `pub struct FieldProvenance { pub source_schema: LightroomSchemaId, pub source_id: String, pub field_path: String, pub artifact: String }`, `pub struct ImportedField<T> { pub value: Option<T>, pub status: ImportStatus, pub provenance: FieldProvenance }`;
  - Dans `rawzelia-contract/src/lightroom_import.rs` : `pub struct UnknownField { pub field_path: String, pub raw_value: serde_json::Value, pub status: ImportStatus, pub provenance: FieldProvenance }`; aucune autre interface du core ne contient `serde_json::Value`;
  - Dans `rawzelia-domain/src/lightroom.rs` : `pub enum CoreValue { Text(String), Integer(i64), Decimal(f64), Boolean(bool), DateTime(String), Path(PathBuf) }`; `rawzelia-domain` réutilise `ImportedField<CoreValue>` et `UnknownField` sans les recopier;
  - `pub struct EditDocument { pub schema_version: u32, pub source_fingerprint: Fingerprint, pub pipeline_version: String, pub adjustments: Vec<Adjustment>, pub community: BTreeMap<String, ImportedField<CoreValue>>, pub lightroom: Vec<ImportedField<CoreValue>>, pub lightroom_unknown: Vec<UnknownField>, pub pro: BTreeMap<String, ImportedField<CoreValue>> }` dans `rawzelia-domain/src/edit.rs`;
  - Dans `rawzelia-contract/src/types.rs` : `pub struct LibraryQuery { pub text: Option<String>, pub date_from: Option<String>, pub date_to: Option<String>, pub collection_id: Option<String>, pub rating: Option<Rating>, pub flag: Option<Flag>, pub label: Option<LabelColor>, pub keyword: Option<String>, pub page: u32, pub page_size: u32 }`, `pub struct LibraryItem { pub asset_id: AssetId, pub relative_path: String, pub rating: Rating, pub flag: Flag, pub label: LabelColor, pub thumbnail: Option<ResourceRef> }`, `pub struct LibraryPage { pub items: Vec<LibraryItem>, pub total: u64, pub page: u32, pub page_size: u32 }`;
  - Dans `rawzelia-contract/src/keymap.rs` : `pub struct KeyStroke { pub key: String, pub modifiers: Vec<String> }`, `pub struct KeyBinding { pub stroke: KeyStroke, pub command: AppCommand }`, `pub struct Keymap { pub version: u16, pub bindings: Vec<KeyBinding> }`, `pub enum FocusContext { Canvas, Panel, TextInput, Menu }`, `pub fn dispatch_keystroke(keymap: &Keymap, stroke: KeyStroke, focus: FocusContext) -> Option<AppCommand>`;
  - `pub struct ImportSessionCommand { pub source_root: PathBuf, pub selected_paths: Vec<PathBuf>, pub destination_root: PathBuf, pub template: String, pub session_name: String }`;
  - `pub struct SetMetadataCommand { pub asset_id: AssetId, pub rating: Rating, pub flag: Flag, pub label: LabelColor, pub keywords: Vec<String> }`;
  - `pub struct SetAdjustmentCommand { pub asset_id: AssetId, pub adjustment: Adjustment }`;
  - `pub struct ApplyPresetCommand { pub asset_ids: Vec<AssetId>, pub preset_id: PresetId }`;
  - `pub struct CopySettingsCommand { pub source_asset_id: AssetId, pub target_asset_ids: Vec<AssetId>, pub sections: Vec<SettingSection> }`;
  - `pub struct RequestPreviewCommand { pub asset_id: AssetId, pub generation: u64, pub target: PreviewTarget }`;
  - `pub struct ExportCommand { pub asset_ids: Vec<AssetId>, pub destination: PathBuf, pub format: ExportFormat, pub profile: ExportProfile, pub quality: u8, pub max_dimension: Option<u32> }`;
  - `pub struct PrintCommand { pub asset_ids: Vec<AssetId>, pub profile: ExportProfile, pub copies: u32 }`;
  - Dans `rawzelia-contract/src/commands.rs`, `pub enum ReferenceChoice { Confirmed }`, `pub struct ReferenceJpegSelection { pub source_id: String, pub jpeg_path: PathBuf, pub fingerprint: Fingerprint, pub choice: ReferenceChoice }`, `pub struct InspectLightroomCommand { pub catalog_path: PathBuf, pub associated_root: Option<PathBuf>, pub reference_jpeg: Option<ReferenceJpegSelection> }`, `pub struct ExecuteLightroomCommand { pub plan_id: String, pub reference_jpeg: Option<ReferenceJpegSelection> }` et `pub struct RollbackLightroomCommand { pub batch_id: String }`;
  - Dans le même module, `pub enum AppCommand { OpenCatalog { path: PathBuf }, ImportSession(ImportSessionCommand), QueryLibrary(LibraryQuery), SetSelection { asset_ids: Vec<AssetId> }, SetMetadata(SetMetadataCommand), SetAdjustment(SetAdjustmentCommand), Undo { asset_id: AssetId }, Redo { asset_id: AssetId }, ApplyPreset(ApplyPresetCommand), CopySettings(CopySettingsCommand), SynchronizeSettings(CopySettingsCommand), RequestPreview(RequestPreviewCommand), Export(ExportCommand), Print(PrintCommand), InspectLightroom(InspectLightroomCommand), ExecuteLightroom(ExecuteLightroomCommand), RollbackLightroom(RollbackLightroomCommand) }`; `rawzelia-app` consomme et orchestre cet enum, mais ne le redéfinit pas;
  - `pub enum JobEventState { Queued, Running, Completed, Failed, Cancelled, Interrupted }`;
  - `pub enum AppEvent { Ready { capabilities: CapabilitySet }, CommandAccepted { request_id: RequestId, job_id: Option<JobId> }, LibraryPageReady { request_id: RequestId, page: LibraryPage }, ImportProgress { job_id: JobId, completed: u32, total: u32, state: JobEventState }, RenderReady { asset_id: AssetId, generation: u64, resource: ResourceRef }, RenderDropped { asset_id: AssetId, generation: u64 }, JobFailed { job_id: JobId, message: String, recoverable: bool }, CatalogChanged { asset_id: Option<AssetId> } }`;
  - `pub trait EventSink: Send + Sync { fn publish(&self, event: AppEvent); }`;
  - `pub enum SourceGuardError { Missing, Active, Locked, Incomplete, Io(String) }` et `pub fn validate_source_guard(path: &Path) -> Result<(), SourceGuardError>` dans `rawzelia-lightroom-import/src/source_guard.rs`. Ce garde ne définit aucun adaptateur, dry-run, workspace ou contrat d’import complet.

- [ ] **Step 1: Write the failing tests for canonical provenance and the minimal source guard**

  Déclarer d’abord dans `crates/rawzelia-contract/Cargo.toml` la dépendance workspace `serde_json`, dans `crates/rawzelia-domain/Cargo.toml` les dépendances workspace vers `rawzelia-contract` et `serde`, et dans `crates/rawzelia-app/Cargo.toml` les dépendances workspace vers `rawzelia-contract`, `rawzelia-domain`, `rawzelia-lightroom-import` et `serde_json`, sans définir encore les types. Ajouter `edit_document_round_trips_namespaces_and_statuses` dans `crates/rawzelia-domain/tests/serialization.rs`, `import_status_and_field_provenance_round_trip` dans `crates/rawzelia-contract/tests/serialization.rs`, `canonical_lightroom_types_compile_across_core_boundary` dans `crates/rawzelia-contract/tests/lightroom_import_contract.rs`, `canonical_lightroom_commands_compile_in_contract` dans `crates/rawzelia-contract/tests/lightroom_commands.rs`, `canonical_types_compile_across_core_and_import` dans `crates/rawzelia-app/tests/lightroom_contract.rs` et `lightroom_reference_jpeg_round_trips_from_selection_to_execute_to_rollback` dans `crates/rawzelia-app/tests/lightroom_command_contract.rs`. Ajouter `locked_catalog_is_rejected_without_mutation` dans `crates/rawzelia-lightroom-import/tests/source_guard.rs`. Les tests doivent vérifier les namespaces `community.*`, `lightroom.*`, `pro.*`, les cinq statuts canoniques, `LightroomSchemaId`, `ImportedField<T>`, `UnknownField`, les champs `source_schema`, `source_id`, `field_path`, `artifact`, la réexportation domaine, la consommation par `validate_source_guard` et le refus d’un fichier `.lrcat.lock` sans création d’une copie de travail. `canonical_types_compile_across_core_and_import` doit importer `FieldProvenance` et `LightroomSchemaId` depuis la réexportation canonique, construire un `ImportedField<CoreValue>` et vérifier que le plan import et le hardening pourront compiler contre exactement ces types. `canonical_lightroom_commands_compile_in_contract` doit compiler `InspectLightroomCommand` avec `reference_jpeg: Some(ReferenceJpegSelection)`, `ExecuteLightroomCommand` avec le même `reference_jpeg` et `RollbackLightroomCommand`; `lightroom_reference_jpeg_round_trips_from_selection_to_execute_to_rollback` doit sérialiser puis désérialiser la séquence `InspectLightroom` → `ExecuteLightroom` → `RollbackLightroom` depuis `rawzelia-contract` dans `rawzelia-app`, en vérifiant le même source ID, fingerprint et `ReferenceJpegSelection` jusqu’à l’appel prévu à `append_artifact`, puis le même batch ID au rollback. Ce test de compilation/sérialisation inter-crates est le point de contrat que le plan import et le hardening étendent sans redéfinir `AppCommand` ni `ReferenceJpegSelection`.

- [ ] **Step 2: Run test to verify it fails**

  Run: `cargo test -p rawzelia-domain --test serialization -- --exact edit_document_round_trips_namespaces_and_statuses`

  Expected: FAIL with `error[E0432]: unresolved import rawzelia_domain::EditDocument` or an equivalent missing-contract error.

  Run: `cargo test -p rawzelia-contract --test serialization -- --exact import_status_and_field_provenance_round_trip`

  Expected: FAIL with `error[E0432]: unresolved import rawzelia_contract::ImportStatus` or an equivalent missing-contract error.

  Run: `cargo test -p rawzelia-contract --test lightroom_import_contract -- --exact canonical_lightroom_types_compile_across_core_boundary`

  Expected: FAIL with `error[E0432]: unresolved import rawzelia_contract::LightroomSchemaId` or an equivalent missing-contract error.

  Run: `cargo test -p rawzelia-contract --test lightroom_commands -- --exact canonical_lightroom_commands_compile_in_contract`

  Expected: FAIL with `error[E0432]: unresolved import rawzelia_contract::InspectLightroomCommand` or an equivalent missing-command-contract error.

  Run: `cargo test -p rawzelia-app --test lightroom_contract -- --exact canonical_types_compile_across_core_and_import`

  Expected: FAIL with `error[E0432]: unresolved import rawzelia_domain::ImportedField` or an equivalent missing-reexport error.

  Run: `cargo test -p rawzelia-app --test lightroom_command_contract -- --exact lightroom_reference_jpeg_round_trips_from_selection_to_rollback`

  Expected: FAIL with `error[E0432]: unresolved import rawzelia_contract::ExecuteLightroomCommand` or an equivalent missing-serialization error.

  Run: `cargo test -p rawzelia-lightroom-import --test source_guard -- --exact locked_catalog_is_rejected_without_mutation`

  Expected: FAIL with `error[E0432]: unresolved import rawzelia_lightroom_import::validate_source_guard`.

- [ ] **Step 3: Implement canonical contracts and the minimal read-only source guard**

  Implémenter d’abord les types partagés sans accès I/O dans `rawzelia-contract/src/types.rs`, puis `LightroomSchemaId`, `ImportStatus`, `FieldProvenance`, `ImportedField<T>` et `UnknownField` dans `rawzelia-contract/src/lightroom_import.rs`. Dans `rawzelia-contract/src/commands.rs`, implémenter et réexporter `ReferenceChoice`, `ReferenceJpegSelection`, `InspectLightroomCommand`, `ExecuteLightroomCommand`, `RollbackLightroomCommand` et l’unique `AppCommand`; `rawzelia-app` importe ces types pour l’orchestration et ne définit aucun enum concurrent. Faire réexporter ces types par `rawzelia-contract/src/lib.rs` et `rawzelia-domain/src/lib.rs` sans les recopier. Le plan import modifiera son `reference.rs` pour que `choose_reference_jpeg` et `associate_reference_jpeg` prennent/retournent le `ReferenceJpegSelection` canonique du contrat; `reference.rs` ne redéfinit ni `ReferenceJpegSelection` ni `ReferenceJpegCommandPayload`. Le plan import portera ce choix depuis `InspectLightroomCommand` vers `ExecuteLightroomCommand`, `ImportPlan` et `append_artifact`, puis exécutera `RollbackLightroomCommand` sur le même bus; il ne redéfinit ni `AppCommand` ni aucun DTO wire. Implémenter les types domaine sans I/O dans `rawzelia-domain`; `serde_json::Value` reste limité à `UnknownField.raw_value`. Implémenter `validate_source_guard` dans `rawzelia-lightroom-import/src/source_guard.rs` avec les contrôles de fichier actif, `.lrcat.lock`, source incomplète et absence de source. Le plan import consomme cette fonction depuis `inspect_source` et mappe `Missing -> SourceMissing`, `Active -> SourceActive`, `Locked -> SourceLocked`, `Incomplete -> SourceCorrupt` et `Io -> StorageFailure` vers `ImportError`; il ne réimplémente aucun contrôle de verrouillage. Garder l’adaptateur, `WorkspaceCopy`, `ImportError`, le dry-run, les artefacts et le parseur Lightroom hors de cette tâche; la sélection JPEG est seulement contractée ici et reste implémentée par les fonctions du plan import.

- [ ] **Step 4: Run test to verify it passes**

  Run: `cargo test -p rawzelia-domain --test serialization -- --exact edit_document_round_trips_namespaces_and_statuses && cargo test -p rawzelia-contract --test serialization -- --exact import_status_and_field_provenance_round_trip && cargo test -p rawzelia-contract --test lightroom_import_contract -- --exact canonical_lightroom_types_compile_across_core_boundary && cargo test -p rawzelia-contract --test lightroom_commands -- --exact canonical_lightroom_commands_compile_in_contract && cargo test -p rawzelia-app --test lightroom_contract -- --exact canonical_types_compile_across_core_and_import && cargo test -p rawzelia-app --test lightroom_command_contract -- --exact lightroom_reference_jpeg_round_trips_from_selection_to_execute_to_rollback && cargo test -p rawzelia-lightroom-import --test source_guard -- --exact locked_catalog_is_rejected_without_mutation && test "$(rg -n 'serde_json::Value' crates/rawzelia-contract/src/lightroom_import.rs | wc -l | tr -d ' ')" = 1`

  Expected: PASS.

- [ ] **Step 5: Commit**

  ```bash
  git add crates/rawzelia-domain crates/rawzelia-contract crates/rawzelia-lightroom-import
  git commit -m "feat: define rawzelia domain and cross-tier contracts"
  ```

### Task 3: Catalogue SQLite, migrations, sidecars et XMP miroir

**Files:**
- Create: `crates/rawzelia-storage/src/catalog.rs`
- Create: `crates/rawzelia-storage/src/migrations.rs`
- Create: `crates/rawzelia-storage/src/sidecar.rs`
- Create: `crates/rawzelia-storage/src/xmp.rs`
- Create: `crates/rawzelia-storage/src/backup.rs`
- Modify: `crates/rawzelia-storage/src/lib.rs`
- Create: `crates/rawzelia-storage/tests/catalog_recovery.rs`
- Create: `crates/rawzelia-storage/tests/sidecar_xmp.rs`
- Create: `crates/rawzelia-storage/tests/reconciliation.rs`
- Create: `crates/rawzelia-storage/migrations/0001_initial.sql`

**Interfaces:**
- Consumes: `AssetRecord`, `AssetId`, `LocationId`, `Fingerprint`, `EditDocument`, `RevisionId`, `LibraryQuery`, `LibraryPage`, `FieldProvenance`, `ImportedField<T>` et `UnknownField` du Task 2.
- Produces:
  - `pub enum StorageError { Io { code: String, message_key: String }, Sql { code: String, message_key: String }, Conflict { fields: Vec<String> }, InvalidBackup { code: String, message_key: String }, Import { code: String, message_key: String, source_id: Option<String>, field_path: Option<String> } }` dans `rawzelia-storage/src/catalog.rs` et réexporté par `src/lib.rs`; c’est l’unique enum de stockage du workspace;
  - `pub struct BackupReport { pub catalog_sha256: [u8; 32], pub sidecar_sha256: BTreeMap<PathBuf, [u8; 32]>, pub entry_count: u64 }`, `pub struct RestoreReport { pub restored_catalog: PathBuf, pub restored_sidecars: u64, pub verified: bool }` dans `rawzelia-storage/src/backup.rs`;
  - `pub enum BackupError { Io(String), HashMismatch, InconsistentCatalog, DestinationExists }`, `pub enum RestoreError { Io(String), InvalidManifest, HashMismatch, DestinationUntouched }`, `pub fn create_backup(catalog: &Path, sidecars: &[PathBuf], destination: &Path) -> Result<BackupReport, BackupError>`, `pub fn restore_backup(backup: &Path, destination: &Path) -> Result<RestoreReport, RestoreError>`;
  - `pub struct XmpSnapshot { pub fields: BTreeMap<String, String> }`, `pub enum Reconciliation { CatalogAuthoritative, SidecarAuthoritative, XmpAuthoritative, Conflict { fields: Vec<String> } }`, `pub struct XmpWriteReport { pub mirrored_fields: u32, pub skipped_fields: u32 }`;
  - `pub trait CatalogStore: Send + Sync { fn open(path: &Path) -> Result<Self, StorageError> where Self: Sized; fn migrate(&self) -> Result<u32, StorageError>; fn upsert_asset(&self, asset: &AssetRecord) -> Result<(), StorageError>; fn find_asset(&self, asset_id: AssetId) -> Result<Option<AssetRecord>, StorageError>; fn remove_asset(&self, asset_id: AssetId) -> Result<(), StorageError>; fn page_library(&self, query: &LibraryQuery) -> Result<LibraryPage, StorageError>; fn append_revision(&self, asset_id: AssetId, document: &EditDocument) -> Result<RevisionId, StorageError>; fn load_edit_document(&self, asset_id: AssetId) -> Result<Option<EditDocument>, StorageError>; fn backup_to(&self, destination: &Path) -> Result<BackupReport, StorageError>; fn restore_from(source: &Path, destination: &Path) -> Result<RestoreReport, StorageError> where Self: Sized; }`; LightroomBatchStore du plan import réutilise exactement `StorageError` et convertit explicitement vers `ImportError` sans nouvelle erreur de stockage;
  - `pub fn read_sidecar(path: &Path) -> Result<Option<EditDocument>, StorageError>`;
  - `pub fn write_sidecar_atomic(path: &Path, document: &EditDocument) -> Result<(), StorageError>`;
  - `pub fn write_xmp_mirror(path: &Path, document: &EditDocument) -> Result<XmpWriteReport, StorageError>`;
  - `pub fn reconcile_sources(catalog: &EditDocument, sidecar: &EditDocument, xmp: &XmpSnapshot) -> Result<Reconciliation, StorageError>`.

- [ ] **Step 1: Write the failing test**

  Ajouter `wal_restore_survives_interrupted_write` dans `catalog_recovery.rs`, `sidecar_write_is_atomic_and_round_trips` dans `sidecar_xmp.rs` et `divergent_sidecar_requires_explicit_resolution` dans `reconciliation.rs`. Les assertions doivent couvrir WAL, transactions, révision immuable, sidecar `.rawzelia`, miroir XMP limité et absence d’écrasement silencieux.

- [ ] **Step 2: Run test to verify it fails**

  Run: `cargo test -p rawzelia-storage --test catalog_recovery -- --exact wal_restore_survives_interrupted_write`

  Expected: FAIL with `error[E0432]: unresolved import rawzelia_storage::CatalogStore`.

- [ ] **Step 3: Implement the minimal catalog and exchange format**

  Créer le schéma SQLite pour assets, locations, métadonnées, collections, keywords, révisions, jobs, exports et cache index. Activer WAL, migrations versionnées et une seule couche d’écriture. Implémenter l’enum unique `StorageError` et les rapports détaillés `BackupReport`/`RestoreReport` dans leurs owners; le plan import réutilisera cette erreur et préservera `code`, `source_id` et `field_path` dans sa conversion `From<StorageError>`. Écrire le sidecar par fichier temporaire, flush, synchronisation et rename atomique. Ne miroiriser en XMP que note, drapeau, couleur, titre, description, droits, orientation, mots-clés et réglages explicitement supportés.

- [ ] **Step 4: Run test to verify it passes**

  Run: `cargo test -p rawzelia-storage --test catalog_recovery -- --exact wal_restore_survives_interrupted_write && cargo test -p rawzelia-storage --test sidecar_xmp && cargo test -p rawzelia-storage --test reconciliation -- --exact divergent_sidecar_requires_explicit_resolution`

  Expected: PASS.

- [ ] **Step 5: Commit**

  ```bash
  git add crates/rawzelia-storage
  git commit -m "feat: add sqlite catalog and sidecar persistence"
  ```

### Task 4: Ingestion SD, volumes, destination et reprise

**Files:**
- Create: `crates/rawzelia-io/src/hash.rs`
- Create: `crates/rawzelia-io/src/volume.rs`
- Create: `crates/rawzelia-io/src/path_template.rs`
- Create: `crates/rawzelia-io/src/ingest.rs`
- Modify: `crates/rawzelia-io/src/lib.rs`
- Create: `crates/rawzelia-io/tests/ingest.rs`
- Create: `crates/rawzelia-io/tests/path_template.rs`
- Create: `crates/rawzelia-io/tests/volume_states.rs`

**Interfaces:**
- Consumes: `AssetRecord`, `Fingerprint`, `AssetAvailability`, `CaptureDate`, `LocationId` et `CatalogStore` des Tasks 2 et 3, ainsi que `ImportSessionCommand` du contrat.
- Produces:
  - `pub enum IoError { SourceMissing, DestinationUnavailable, HashMismatch, Cancelled, PermissionDenied, Io(String) }`;
  - `pub struct PathTemplate(String);`
  - `impl PathTemplate { pub fn default() -> Self; pub fn render(&self, date: CaptureDate, session_name: &str, original_name: &str) -> Result<PathBuf, IoError>; }` avec défaut `Photos/AAAA/AAAA-MM-JJ Nom-session/`;
  - `pub struct ImportRequest { pub source_root: PathBuf, pub selected_files: Vec<PathBuf>, pub destination_root: PathBuf, pub template: PathTemplate, pub session_name: String }`;
  - `pub struct PlannedIngest { pub asset_id: AssetId, pub source: PathBuf, pub destination: PathBuf, pub fingerprint: Fingerprint }`, `pub struct IngestPlan { pub items: Vec<PlannedIngest>, pub destination_root: PathBuf }`;
  - `pub fn plan_ingest(request: &ImportRequest) -> Result<IngestPlan, IoError>`;
  - `pub fn compute_fingerprint(path: &Path) -> Result<Fingerprint, IoError>`;
  - `pub enum ImportProgressState { Pending, Copying, Verified, Duplicate, Failed }`, `pub struct ImportProgress { pub completed: u32, pub total: u32, pub current: Option<PathBuf>, pub state: ImportProgressState }`;
  - `pub struct ImportSummary { pub imported: u32, pub duplicates: u32, pub failed: u32, pub resumed: bool }`, `pub fn execute_ingest(plan: &IngestPlan, progress: &mut dyn FnMut(ImportProgress), should_cancel: &dyn Fn() -> bool) -> Result<ImportSummary, IoError>`;
  - `pub struct VolumeInfo { pub root: PathBuf, pub location_id: LocationId, pub availability: AssetAvailability }`, `pub fn classify_volume(path: &Path) -> Result<VolumeInfo, IoError>`.

- [ ] **Step 1: Write the failing test**

  Ajouter `default_template_renders_session_path`, `duplicate_ingest_is_idempotent_and_interruption_is_resumable` et `offline_volume_is_not_marked_present`. Utiliser des fichiers temporaires, des tailles et hashes contrôlés, une interruption après le premier fichier et deux exécutions du même plan d’ingestion.

- [ ] **Step 2: Run test to verify it fails**

  Run: `cargo test -p rawzelia-io --test ingest -- --exact duplicate_ingest_is_idempotent_and_interruption_is_resumable`

  Expected: FAIL with `error[E0432]: unresolved import rawzelia_io::execute_ingest`.

- [ ] **Step 3: Implement verified copy and resumable ingestion**

  Rendre `IngestPlan` déterministe. Copier vers un fichier temporaire, vérifier taille puis SHA-256, renommer atomiquement et seulement ensuite produire le résultat validé. Réutiliser un fichier déjà identique, déclarer les doublons, préserver les états `Missing`, `Moved`, `VolumeOffline` et `PermissionDenied`, et exposer un progrès persistable sans déclarer un import partiel terminé. Réserver le nom `ImportPlan` au dry-run Lightroom du plan import.

- [ ] **Step 4: Run test to verify it passes**

  Run: `cargo test -p rawzelia-io --test ingest && cargo test -p rawzelia-io --test path_template && cargo test -p rawzelia-io --test volume_states`

  Expected: PASS.

- [ ] **Step 5: Commit**

  ```bash
  git add crates/rawzelia-io
  git commit -m "feat: add verified sd ingestion"
  ```

### Task 5: Pipeline NEF D750 CPU, DCP et profils objectif

**Files:**
- Create: `crates/rawzelia-imaging/src/raw.rs`
- Create: `crates/rawzelia-imaging/src/color.rs`
- Create: `crates/rawzelia-imaging/src/render_cpu.rs`
- Create: `crates/rawzelia-imaging/src/profiles.rs`
- Modify: `crates/rawzelia-imaging/src/lib.rs`
- Create: `crates/rawzelia-imaging/tests/golden_d750.rs`
- Create: `crates/rawzelia-imaging/tests/corrupt_input.rs`
- Create: `crates/rawzelia-imaging/tests/color_profiles.rs`
- Create: `crates/rawzelia-imaging/tests/lens_profiles.rs`
- Create: `tests/fixtures/nef/manifest.toml`
- Create: `tests/fixtures/nef/README.md`

**Interfaces:**
- Consumes: `EditDocument`, `Adjustment`, `Fingerprint`, `AssetId` et `AssetMetadata` du domaine ; `RAWZELIA_FIXTURES_DIR` est la seule racine, le manifest suivi `tests/fixtures/nef/manifest.toml` résout les NEF et golden sous `$RAWZELIA_FIXTURES_DIR/nef/`.
- Produces:
  - `pub enum ImagingError { Decode(String), CorruptInput, MissingProfile(String), ProfileNotRedistributable(String), Unsupported(String), Io(String) }`;
  - `pub enum DecodeTarget { Metadata, EmbeddedPreview, Proxy, Full }`, `pub struct DecodeRequest { pub path: PathBuf, pub target: DecodeTarget }`, `pub struct PreviewRef { pub cache_path: PathBuf, pub width: u32, pub height: u32 }`, `pub struct DecodedRaw { /* opaque */ }`;
  - `pub struct RawMetadata { pub width: u32, pub height: u32, pub orientation: u16, pub camera: String, pub lens: Option<String>, pub embedded_preview: Option<PreviewRef> }`;
  - `pub struct DcpProfileRef { pub profile_id: String, pub sha256: [u8; 32], pub redistributable: bool }`, `pub struct CameraProfile { pub id: String, pub dcp: DcpProfileRef }`, `pub struct WorkingSpace { pub name: String }`, `pub struct DisplayProfile { pub name: String }`;
  - `pub enum ReferenceLens { Nikon16_35F4GVR, Nikon24_70F28EEDVR, Sigma50F14Art, Samyang85F14Manual }`, `pub struct ManualLensCorrection { pub distortion: f32, pub vignetting: f32, pub chromatic_aberration: f32 }`;
  - `pub enum LensProfileSelection { AutoFromExif { fallback: Option<ReferenceLens> }, Explicit { lens: ReferenceLens }, Manual { base: Option<ReferenceLens>, correction: ManualLensCorrection }, None }`, `pub struct LensProfile { pub profile_id: String, pub selection: LensProfileSelection }`;
  - `pub fn select_lens_profile(asset_id: AssetId, metadata: &RawMetadata, selection: LensProfileSelection) -> Result<LensProfile, ImagingError>`; `Explicit { lens: Samyang85F14Manual }` and `Manual { base: None, ... }` are valid without EXIF.
  - `pub trait RawDecoder: Send + Sync { fn metadata(&self, path: &Path) -> Result<RawMetadata, ImagingError>; fn decode(&self, request: DecodeRequest) -> Result<DecodedRaw, ImagingError>; }`;
  - `pub struct ColorProfiles { pub camera: CameraProfile, pub working: WorkingSpace, pub display: DisplayProfile, pub export: ExportProfile, pub lens: LensProfile }`;
  - `pub enum RenderTarget { Proxy { max_pixels: u32 }, Tile { x: u32, y: u32, width: u32, height: u32 }, Export { width: u32, height: u32 } }`, `pub struct RenderedImage { pub width: u32, pub height: u32, pub pixels: Vec<u8>, pub profile: String }`;
  - `pub struct GoldenTolerance { pub max_abs_error: u8, pub max_changed_pixels: u32 }`, `pub struct GoldenReport { pub max_abs_error: u8, pub changed_pixels: u32, pub passed: bool }`;
  - `pub fn render_cpu(raw: &DecodedRaw, document: &EditDocument, profiles: &ColorProfiles, target: RenderTarget) -> Result<RenderedImage, ImagingError>`;
  - `pub fn compare_images(expected: &RenderedImage, actual: &RenderedImage, tolerance: &GoldenTolerance) -> Result<GoldenReport, ImagingError>`.

- [ ] **Step 1: Write the failing test**

  Ajouter `fixture_manifest_is_complete`, `d750_nef_renders_reference_scene`, `truncated_nef_returns_classified_error`, `icc_profile_is_recorded_in_render_key` et `samyang_manual_selection_works_without_exif` dans les tests indiqués. Le manifest `tests/fixtures/nef/manifest.toml` doit imposer `expected_nef_count = 100`, les quatre objectifs nommés, les chemins golden sous `nef/golden/` et les checksums attendus. Tous les chemins externes sont relatifs à l’unique `RAWZELIA_FIXTURES_DIR`; le test Samyang doit utiliser `lens = None`, une sélection explicite, une correction manuelle et un DCP non redistribuable rejeté sans fuite de fichier.

- [ ] **Step 2: Run test to verify it fails**

  Run: `RAWZELIA_FIXTURES_DIR="$PWD/fixtures/rawzelia-v0.1" cargo test -p rawzelia-imaging --test golden_d750 -- --exact d750_nef_renders_reference_scene`

  Expected: FAIL with `error[E0432]: unresolved import rawzelia_imaging::render_cpu` or the equivalent missing-decoder error.

- [ ] **Step 3: Implement the CPU oracle, DCP and lens profile boundary**

  Encapsuler LibRaw dans `raw.rs`. Décoder les NEF D750, puis appliquer dans l’ordre de la spec niveaux noirs/blancs, balance des blancs capteur, dématriçage Bayer, conversion caméra, tonalité, réglages essentiels, correction objectif ciblée, géométrie, ICC et encodage de cible. Implémenter `select_lens_profile` dans `profiles.rs` pour les quatre objectifs, le Samyang sans EXIF et la correction manuelle. Charger un `DcpProfileRef` seulement si `redistributable == true` ou si le profil est fourni par l’utilisateur. Enregistrer DCP, profil caméra, profil d’affichage, profil d’export et profil objectif dans les clés et révisions. Rendre l’absence de profil visible sans correction silencieuse.

- [ ] **Step 4: Run test to verify it passes**

  Run: `RAWZELIA_FIXTURES_DIR="$PWD/fixtures/rawzelia-v0.1" cargo test -p rawzelia-imaging --test golden_d750 && cargo test -p rawzelia-imaging --test corrupt_input && cargo test -p rawzelia-imaging --test color_profiles && cargo test -p rawzelia-imaging --test lens_profiles`

  Expected: PASS.

- [ ] **Step 5: Commit**

  ```bash
    git add crates/rawzelia-imaging tests/fixtures/nef/manifest.toml tests/fixtures/nef/README.md
  git commit -m "feat: add d750 cpu imaging pipeline"
  ```

### Task 6: Previews, cache, proxy GPU et fallback CPU

**Files:**
- Create: `crates/rawzelia-imaging/src/cache.rs`
- Create: `crates/rawzelia-imaging/src/render_gpu.rs`
- Modify: `crates/rawzelia-imaging/src/lib.rs`
- Create: `crates/rawzelia-imaging/tests/render_generation.rs`
- Create: `crates/rawzelia-imaging/tests/cache_key.rs`
- Create: `crates/rawzelia-imaging/tests/gpu_cpu_parity.rs`

**Interfaces:**
- Consumes: `render_cpu`, `ColorProfiles`, `EditDocument`, `Fingerprint`, `RenderTarget` du Task 5.
- Produces:
  - `pub struct CacheKey { pub asset_id: AssetId, pub fingerprint: Fingerprint, pub pipeline_version: String, pub document_hash: [u8; 32], pub target: RenderTarget, pub color_key: String, pub lens_profile_key: String, pub dcp_profile_key: String }`;
  - `pub fn make_cache_key(asset_id: AssetId, fingerprint: Fingerprint, document: &EditDocument, target: RenderTarget, profiles: &ColorProfiles) -> CacheKey`;
  - `pub struct RenderRequest { pub asset_id: AssetId, pub source: PathBuf, pub document: EditDocument, pub generation: u64, pub target: RenderTarget }`;
  - `pub struct RenderToken(pub u64);`;
  - `pub struct CachedResource { pub ref_id: String, pub path: PathBuf, pub width: u32, pub height: u32 }`;
  - `pub trait InteractiveRenderer: Send { fn submit(&mut self, request: RenderRequest) -> RenderToken; fn poll(&mut self, token: RenderToken) -> Result<RenderPoll, ImagingError>; fn render_cpu_fallback(&self, request: RenderRequest) -> Result<RenderedImage, ImagingError>; }`;
  - `pub enum RenderPoll { Pending, Ready { generation: u64, resource: CachedResource }, Dropped { generation: u64 }, Failed(ImagingError) }`.

- [ ] **Step 1: Write the failing test**

  Ajouter `cache_key_changes_when_document_or_profile_changes`, `lens_profile_and_dcp_change_cache_key`, `stale_generation_is_not_published_and_cpu_fallback_is_available` et `gpu_proxy_matches_cpu_with_versioned_tolerance`.

- [ ] **Step 2: Run test to verify it fails**

  Run: `cargo test -p rawzelia-imaging --test render_generation -- --exact stale_generation_is_not_published_and_cpu_fallback_is_available`

  Expected: FAIL with `error[E0432]: unresolved import rawzelia_imaging::InteractiveRenderer`.

- [ ] **Step 3: Implement bounded cache and GPU proxy**

  Mettre en cache aperçu JPEG embarqué, proxy, tuiles et rendu exporté avec quota LRU et clé complète. Implémenter le proxy `wgpu` avec compilation de pipeline en arrière-plan, génération monotone, publication conditionnelle et backend Metal attendu. Retourner `Pending`, `Ready`, `Dropped` ou `Failed` sans publier de génération obsolète. Le fallback CPU reste obligatoire et l’export ne dépend pas du GPU.

- [ ] **Step 4: Run test to verify it passes**

  Run: `cargo test -p rawzelia-imaging --test cache_key && cargo test -p rawzelia-imaging --test render_generation && cargo test -p rawzelia-imaging --test gpu_cpu_parity`

  Expected: PASS. On a machine sans backend GPU utilisable, `gpu_cpu_parity` doit passer par le fallback CPU et signaler le backend absent comme état diagnostique.

- [ ] **Step 5: Commit**

  ```bash
  git add crates/rawzelia-imaging
  git commit -m "feat: add preview cache and gpu fallback"
  ```

### Task 7: Orchestration applicative, suppression et dispatch typé

**Files:**
- Create: `crates/rawzelia-app/src/config.rs`
- Create: `crates/rawzelia-app/src/runtime.rs`
- Create: `crates/rawzelia-app/src/jobs.rs`
- Create: `crates/rawzelia-app/src/use_cases.rs`
- Modify: `crates/rawzelia-app/src/lib.rs`
- Create: `crates/rawzelia-app/tests/use_cases.rs`
- Create: `crates/rawzelia-app/tests/job_recovery.rs`
- Create: `crates/rawzelia-app/tests/delete_commands.rs`
- Modify: `crates/rawzelia-contract/src/commands.rs`
- Modify: `crates/rawzelia-io/src/lib.rs`
- Create: `crates/rawzelia-io/src/trash.rs`
- Create: `crates/rawzelia-io/tests/trash.rs`

**Interfaces:**
- Consumes: `CatalogStore`, `plan_ingest`, `execute_ingest`, `RawDecoder`, `render_cpu`, `InteractiveRenderer`, `AppCommand`, `InspectLightroomCommand`, `ExecuteLightroomCommand`, `RollbackLightroomCommand`, `AppEvent`, `EventSink`, `AssetRecord`, `DeleteAssetsCommand` et `TrashBackend` des Tasks 2 à 6.
- Produces:
  - `pub enum DeleteMode { CatalogOnly, MoveToTrash }`, `pub struct DeleteAssetsCommand { pub asset_ids: Vec<AssetId>, pub mode: DeleteMode, pub confirmation: bool }`, ajoutée à l’unique `AppCommand` de `rawzelia-contract/src/commands.rs` sous `DeleteAssets(DeleteAssetsCommand)`; `rawzelia-app` ne redéfinit pas l’enum;
  - `pub enum TrashError { UnsupportedPlatform, PermissionDenied, Io(String) }`, `pub trait TrashBackend: Send + Sync { fn move_to_trash(&self, path: &Path) -> Result<(), TrashError>; }` dans `rawzelia-io/src/trash.rs`;
  - `pub enum AppError { Storage(StorageError), Io(IoError), Imaging(ImagingError), Delete(String), Validation(String) }`, `pub struct DeleteSummary { pub removed_from_catalog: u32, pub moved_to_trash: u32 }`;
  - `pub fn delete_assets(command: &DeleteAssetsCommand, catalog: &dyn CatalogStore, trash: &dyn TrashBackend) -> Result<DeleteSummary, AppError>` dans `rawzelia-app/src/use_cases.rs`. Refuser `confirmation == false`; en mode `CatalogOnly`, retirer uniquement l’index ; en mode `MoveToTrash`, déplacer via `TrashBackend` avant de retirer l’index.
  - `pub struct AppConfig { pub catalog_path: PathBuf, pub cache_root: PathBuf, pub offline: bool }`;
  - `pub struct AppRuntime { /* opaque */ }`;
  - `pub fn build_runtime(config: AppConfig) -> Result<AppRuntime, AppError>`;
  - `pub struct SubscriptionId(pub Uuid);`, `impl AppRuntime { pub fn execute(&self, command: AppCommand) -> Result<CommandReceipt, AppError>; pub fn subscribe(&self, sink: Arc<dyn EventSink>) -> SubscriptionId; pub fn recover_interrupted_jobs(&self) -> Result<Vec<JobId>, AppError>; }`;
  - `pub struct CommandReceipt { pub request_id: RequestId, pub job_id: Option<JobId> }`;
  - `pub type JobState = JobEventState`; `AppRuntime::recover_interrupted_jobs` est l’unique propriétaire de la transition `Running -> Interrupted`.

- [ ] **Step 1: Write the failing tests for runtime recovery and destructive deletion**

  Ajouter `import_develop_export_dispatches_events` et `running_job_becomes_interrupted_after_restart` dans `use_cases.rs`. Ajouter `delete_catalog_only_requires_confirmation`, `move_to_trash_uses_backend_after_confirmation` et `missing_original_does_not_remove_catalog_record` dans `delete_commands.rs`. Ajouter `trash_backend_receives_only_confirmed_paths` dans `crates/rawzelia-io/tests/trash.rs`. Les tests doivent couvrir confirmation obligatoire, retrait catalogue seul, corbeille abstraite, original absent et reprise explicite d’un job `Running`.

- [ ] **Step 2: Run test to verify it fails**

  Run: `cargo test -p rawzelia-app --test delete_commands -- --exact delete_catalog_only_requires_confirmation`

  Expected: FAIL with `error[E0432]: unresolved import rawzelia_app::delete_assets` or an equivalent missing-command error.

- [ ] **Step 3: Implement the minimal runtime, recovery and deletion boundary**

  Orchestrer chaque commande par un cas d’usage unique. Importer `AppCommand`, `InspectLightroomCommand`, `ExecuteLightroomCommand`, `ReferenceJpegSelection` et `RollbackLightroomCommand` depuis `rawzelia-contract`; aucune définition d’`AppCommand`, de `ExecuteLightroomCommand` ou de `ReferenceJpegSelection` n’est créée dans `rawzelia-app`. Mapper `InspectLightroomCommand` vers `LightroomImportRequest`, puis `ExecuteLightroomCommand` vers `ImportPlan`; la transaction du plan import transforme le `ReferenceJpegSelection` au même fingerprint en artefact et appelle `append_artifact`, puis `RollbackLightroomCommand` retire cet artefact par `batch_id`. Persister les jobs, publier les événements versionnés, borner la concurrence et faire passer les pixels par le cache local. Au démarrage, convertir `Running` en `Interrupted` dans `AppRuntime::recover_interrupted_jobs` et ne reprendre qu’après décision explicite. Pour la suppression, exiger la confirmation, appeler `TrashBackend::move_to_trash` en mode `MoveToTrash`, puis retirer l’asset du catalogue après succès. En cas d’échec de corbeille, conserver l’asset au catalogue. Toute commande venue d’un raccourci, menu, bouton, bridge Tauri ou palette utilise le même `AppCommand`.

- [ ] **Step 4: Run test to verify it passes**

  Run: `cargo test -p rawzelia-app --test use_cases && cargo test -p rawzelia-app --test job_recovery && cargo test -p rawzelia-app --test delete_commands && cargo test -p rawzelia-io --test trash`

  Expected: PASS.

- [ ] **Step 5: Commit**

  ```bash
  git add crates/rawzelia-app
  git commit -m "feat: orchestrate typed rawzelia jobs"
  ```

### Task 8: Shell Tauri 2, bridge typé, spike et mode Bibliothèque

**Files:**
- Create: `crates/rawzelia-desktop/tauri.conf.json`
- Create: `crates/rawzelia-desktop/capabilities/default.json`
- Create: `crates/rawzelia-desktop/src/state.rs`
- Create: `crates/rawzelia-desktop/src/shell_gate.rs`
- Create: `crates/rawzelia-desktop/src/commands.rs`
- Create: `crates/rawzelia-desktop/src/events.rs`
- Create: `crates/rawzelia-desktop/src/egui_shell.rs`
- Create: `crates/rawzelia-desktop/src/bin/egui_shell.rs`
- Modify: `crates/rawzelia-desktop/Cargo.toml`
- Modify: `crates/rawzelia-desktop/src/lib.rs`
- Modify: `crates/rawzelia-desktop/src/main.rs`
- Create: `crates/rawzelia-desktop/ui/package.json`
- Create: `crates/rawzelia-desktop/ui/package-lock.json`
- Create: `crates/rawzelia-desktop/ui/tsconfig.json`
- Create: `crates/rawzelia-desktop/ui/vite.config.ts`
- Create: `crates/rawzelia-desktop/ui/src/main.ts`
- Create: `crates/rawzelia-desktop/ui/src/contracts.ts`
- Create: `crates/rawzelia-desktop/ui/src/bridge.ts`
- Create: `crates/rawzelia-desktop/ui/src/library.ts`
- Create: `crates/rawzelia-desktop/ui/src/layout.ts`
- Create: `crates/rawzelia-desktop/ui/src/keymap.ts`
- Create: `crates/rawzelia-desktop/ui/src/styles.css`
- Create: `crates/rawzelia-desktop/ui/src/library.test.ts`
- Create: `crates/rawzelia-desktop/ui/src/lightroom_import.test.ts`
- Create: `crates/rawzelia-desktop/tests/lightroom_command_contract.rs`
- Create: `crates/rawzelia-desktop/tests/tauri_commands.rs`
- Create: `crates/rawzelia-desktop/tests/tauri_smoke.rs`
- Create: `crates/rawzelia-desktop/tests/egui_smoke.rs`

**Interfaces:**
- Consumes: `AppRuntime`, `AppCommand`, `AppEvent`, `CommandReceipt`, `ResourceRef` et `EventSink` du Task 7.
- Produces:
  - `pub struct DesktopState { pub runtime: Arc<AppRuntime>, pub resource_root: PathBuf }`, `pub struct ResourceDescriptor { pub resource: ResourceRef, pub cache_path: PathBuf }`;
  - `#[tauri::command] pub async fn execute_command(command: AppCommand, state: State<'_, DesktopState>) -> Result<CommandReceipt, AppError>`;
  - `#[tauri::command] pub fn read_resource(resource: ResourceRef, state: State<'_, DesktopState>) -> Result<ResourceDescriptor, AppError>`;
  - `pub const EVENT_APP_STATE: &str = "rawzelia://app-state";`, `pub const EVENT_JOB_PROGRESS: &str = "rawzelia://job-progress";`, `pub const EVENT_RENDER_READY: &str = "rawzelia://render-ready"`;
  - `pub struct ShellGateReport { pub bridge_p95_ms: u64, pub library_smoke_ok: bool, pub focus_ok: bool, pub local_assets_only: bool }`, `pub enum ShellGateError { BridgeTooSlow, LibrarySmokeFailed, FocusFailed, RemoteAssetDetected }`, `pub fn assert_tauri_gate(report: &ShellGateReport) -> Result<(), ShellGateError>` dans `src/shell_gate.rs`; `TAURI_SMOKE_MAX_MS` borne le smoke à 2 000 ms;
  - Dans `crates/rawzelia-desktop/Cargo.toml` : feature optionnelle `egui-shell`, dépendances `egui`/`eframe` optionnelles aux versions majeures validées au bootstrap, et binaire `egui_shell` avec `required-features = ["egui-shell"]`;
  - `pub enum DesktopMode { Library, Development }`, `pub struct EguiShellModel { pub mode: DesktopMode, pub selected: Vec<AssetId>, pub last_event: Option<AppEvent> }`, `pub struct RenderReport { pub frame_pixels: u64, pub mode: DesktopMode }`, `pub enum DesktopError { Command(String), Render(String) }`;
  - `pub fn navigate_egui(model: &mut EguiShellModel, command: AppCommand) -> Option<AppEvent>`, `pub fn render_egui_frame(model: &EguiShellModel, context: &egui::Context) -> Result<RenderReport, DesktopError>` et `pub fn run_egui_shell() -> Result<(), DesktopError>` dans `src/egui_shell.rs`; `src/bin/egui_shell.rs` appelle cette fonction et ne définit aucun contrat parallèle;
  - `export type AssetId = string;`, `export type AppCommand = { type: "openCatalog"; path: string } | { type: "importSession"; payload: ImportSessionPayload } | { type: "queryLibrary"; payload: LibraryQuery } | { type: "setSelection"; assetIds: AssetId[] } | { type: "setMetadata"; payload: SetMetadataPayload } | { type: "setAdjustment"; payload: SetAdjustmentPayload } | { type: "undo"; assetId: AssetId } | { type: "redo"; assetId: AssetId } | { type: "applyPreset"; payload: ApplyPresetPayload } | { type: "copySettings"; payload: CopySettingsPayload } | { type: "synchronizeSettings"; payload: CopySettingsPayload } | { type: "requestPreview"; payload: RequestPreviewPayload } | { type: "export"; payload: ExportPayload } | { type: "print"; payload: PrintPayload } | { type: "inspectLightroom"; payload: InspectLightroomPayload } | { type: "executeLightroom"; payload: ExecuteLightroomPayload } | { type: "rollbackLightroom"; payload: RollbackLightroomPayload } | { type: "deleteAssets"; payload: DeleteAssetsPayload };`;
  - `export interface Fingerprint { size: number; sha256: number[] }`, `export interface ReferenceJpegSelection { source_id: string; jpeg_path: string; fingerprint: Fingerprint; choice: "confirmed" }`, `export interface ImportSessionPayload { sourceRoot: string; selectedPaths: string[]; destinationRoot: string; template: string; sessionName: string }`, `export interface LibraryQuery { text?: string; dateFrom?: string; dateTo?: string; collectionId?: string; rating?: string; flag?: string; label?: string; keyword?: string; page: number; pageSize: number }`, `export interface DeleteAssetsPayload { assetIds: AssetId[]; mode: "catalogOnly" | "moveToTrash"; confirmation: boolean }`;
  - `export interface SetMetadataPayload { assetId: AssetId; rating: string; flag: string; label: string; keywords: string[] }`, `export interface SetAdjustmentPayload { assetId: AssetId; adjustment: Adjustment }`, `export interface ApplyPresetPayload { assetIds: AssetId[]; presetId: string }`, `export interface CopySettingsPayload { sourceAssetId: AssetId; targetAssetIds: AssetId[]; sections: SettingSection[] }`, `export interface RequestPreviewPayload { assetId: AssetId; generation: number; target: PreviewTarget }`, `export interface ExportPayload { assetIds: AssetId[]; destination: string; format: ExportFormat; profile: ExportProfile; quality: number; maxDimension?: number }`, `export interface PrintPayload { assetIds: AssetId[]; profile: ExportProfile; copies: number }`, `export interface InspectLightroomPayload { catalogPath: string; associatedRoot?: string; reference_jpeg?: ReferenceJpegSelection }`, `export interface ExecuteLightroomPayload { plan_id: string; reference_jpeg?: ReferenceJpegSelection }`, `export interface RollbackLightroomPayload { batch_id: string }`;
  - `export interface CapabilitySet { contractVersion: number; capabilities: string[] }`, `export interface CommandReceipt { requestId: string; jobId?: string }`, `export type Adjustment = { kind: "exposure"; value: number } | { kind: "contrast"; value: number } | { kind: "highlights"; value: number } | { kind: "shadows"; value: number } | { kind: "whiteBalance"; temperature: number; tint: number } | { kind: "crop"; x: number; y: number; width: number; height: number } | { kind: "rotation"; degrees: number } | { kind: "straighten"; degrees: number } | { kind: "lensCorrection"; profile: string; amount: number }`, `export type SettingSection = "tone" | "whiteBalance" | "geometry" | "lensCorrection" | "all"`, `export type PreviewTarget = "grid" | "loupe" | "oneToOne"`, `export type ExportFormat = "jpegFull" | "jpegWeb" | "png"`, `export type ExportProfile = { kind: "srgb" } | { kind: "icc"; path: string }` dans `ui/src/contracts.ts`;
  - `export type AppEvent = { type: "ready"; payload: { capabilities: CapabilitySet } } | { type: "commandAccepted"; payload: { requestId: string; jobId?: string } } | { type: "libraryPageReady"; payload: { requestId: string; page: LibraryPage } } | { type: "importProgress"; payload: { jobId: string; completed: number; total: number; state: string } } | { type: "renderReady"; payload: { assetId: AssetId; generation: number; resource: ResourceRef } } | { type: "renderDropped"; payload: { assetId: AssetId; generation: number } } | { type: "jobFailed"; payload: { jobId: string; message: string; recoverable: boolean } } | { type: "catalogChanged"; payload: { assetId?: AssetId } };`, `export type Unsubscribe = () => void;` dans `ui/src/contracts.ts`;
  - `export interface KeyStroke { key: string; modifiers: string[] }`, `export interface KeyBinding { stroke: KeyStroke; command: AppCommand }`, `export interface Keymap { version: number; bindings: KeyBinding[] }`, `export type FocusContext = "canvas" | "panel" | "textInput" | "menu"`, `export function dispatchKeystroke(keymap: Keymap, stroke: KeyStroke, focus: FocusContext): AppCommand | undefined`, `export interface ResourceRef { id: string; mime: string; width: number; height: number }`, `export interface LibraryPage { items: LibraryItem[]; total: number; page: number; pageSize: number }`, `export interface LibraryItem { assetId: AssetId; relativePath: string; rating: string; flag: string; label: string; thumbnail?: ResourceRef }`;
  - `export function buildInspectLightroomCommand(payload: InspectLightroomPayload): AppCommand`, `export function buildExecuteLightroomCommand(payload: ExecuteLightroomPayload): AppCommand`, `export function buildRollbackLightroomCommand(payload: RollbackLightroomPayload): AppCommand`, `export function invokeCommand(command: AppCommand): Promise<CommandReceipt>` et `export function subscribeToAppEvents(listener: (event: AppEvent) => void): Unsubscribe` dans `ui/src/bridge.ts`; le bridge transmet le même `reference_jpeg` et fingerprint sans route HTTP ni contrat parallèle;
  - `export function renderLibraryPage(page: LibraryPage, container: HTMLElement): void`, `export function updateFilmstrip(selection: AssetId[], container: HTMLElement): void` et `export function applyKeymap(key: Keymap, dispatch: (command: AppCommand) => Promise<CommandReceipt>): void` côté TypeScript; `applyKeymap` appelle le même `dispatchKeystroke` que le menu et la palette.

- [ ] **Step 1: Write the failing test**

  Déclarer d’abord dans `crates/rawzelia-desktop/Cargo.toml` les features exactes `default = ["community"]`, `community = []` et `egui-shell = ["dep:egui", "dep:eframe"]`, les dépendances optionnelles `egui`/`eframe` aux versions majeures validées au bootstrap et le binaire `egui_shell` avec `required-features = ["egui-shell"]`, sans écrire encore son module. Ajouter dans `crates/rawzelia-desktop/ui/package.json` les scripts exacts `"check": "tsc --noEmit"` et `"test": "vitest run"`. Ajouter `library_command_is_invoked_through_tauri_bridge` dans `crates/rawzelia-desktop/ui/src/library.test.ts`, `lightroom_reference_jpeg_bridge_and_execute_and_rollback_use_typed_bus` dans `crates/rawzelia-desktop/ui/src/lightroom_import.test.ts`, en important les trois builders et `invokeCommand` depuis `./bridge`, et `desktop_lightroom_flow_uses_canonical_commands_through_append_artifact_and_rollback` dans `crates/rawzelia-desktop/tests/lightroom_command_contract.rs`. Le test UI doit construire `InspectLightroomPayload`, `ExecuteLightroomPayload` et `RollbackLightroomPayload` avec le même `reference_jpeg` et fingerprint, construire les trois variantes sur le même `AppCommand` et vérifier qu’aucun appel HTTP n’est créé. Le test desktop doit compiler/sérialiser la séquence inspect → execute → app mapping vers `ImportPlan` → `append_artifact` → rollback et vérifier que le même fingerprint est associé puis retiré par `batch_id`. Ajouter `desktop_command_rejects_unknown_resource` dans `crates/rawzelia-desktop/tests/tauri_commands.rs`; ce test doit refuser une ressource qui n’est pas dans le cache autorisé. Ajouter `tauri_smoke_stays_local_and_bounded` dans `crates/rawzelia-desktop/tests/tauri_smoke.rs`, avec un timeout de 2 000 ms et une assertion sur `ShellGateReport`. Ajouter `egui_navigation_and_render_smoke` dans `crates/rawzelia-desktop/tests/egui_smoke.rs`, compilé seulement avec `egui-shell`, qui réutilise `AppCommand::QueryLibrary`, `AppEvent::LibraryPageReady`, vérifie la navigation Bibliothèque/Développement et exige `frame_pixels > 0`; ce test n’est exécuté que si le gate Tauri échoue.

- [ ] **Step 2: Run test to verify it fails**

  Run: `npm test --prefix crates/rawzelia-desktop/ui -- --run src/library.test.ts`

  Expected: FAIL with `Failed to resolve import ./bridge` (or the equivalent missing-module error).

  Run: `npm test --prefix crates/rawzelia-desktop/ui -- --run src/lightroom_import.test.ts`

  Expected: FAIL with `Failed to resolve import ./bridge` or a missing `reference_jpeg`/execute/rollback command builder error.

  Run from `crates/rawzelia-desktop`: `cargo test --test tauri_commands -- --exact desktop_command_rejects_unknown_resource`

  Expected: FAIL with `error[E0432]: unresolved import rawzelia_desktop::commands`.

  Run from `crates/rawzelia-desktop`: `cargo test --test lightroom_command_contract -- --exact desktop_lightroom_flow_uses_canonical_commands_through_append_artifact_and_rollback`

  Expected: FAIL with `error[E0432]: unresolved import rawzelia_contract::ExecuteLightroomCommand` or an equivalent missing typed-flow error.

  Run from `crates/rawzelia-desktop`: `cargo test --test tauri_smoke -- --exact tauri_smoke_stays_local_and_bounded`

  Expected: FAIL with `error[E0432]: unresolved import rawzelia_desktop::shell_gate`.

  If and only if the Tauri gate is FAIL, run from repository root: `cargo test -p rawzelia-desktop --features egui-shell --test egui_smoke -- --exact egui_navigation_and_render_smoke`

  Expected: FAIL with `error[E0432]: unresolved import rawzelia_desktop::navigate_egui`.

- [ ] **Step 3: Implement the minimal Tauri bridge and bounded shell gate**

  Configurer Tauri 2 pour une cible `aarch64-apple-darwin`, permissions locales minimales et `frontendDist` vers `ui/dist`. Utiliser `invoke` et `listen` Tauri, jamais fetch HTTP. Le bridge expose les builders `buildInspectLightroomCommand`, `buildExecuteLightroomCommand` et `buildRollbackLightroomCommand`, transmet le même `reference_jpeg` et fingerprint aux commandes canoniques, puis le desktop vérifie inspect → execute → app → `append_artifact` → rollback; aucune commande Lightroom ni aucun `ReferenceJpegSelection` n’est redéfini dans `rawzelia-app` ou le desktop. Construire le mode Bibliothèque avec navigation gauche, grille, filmstrip, recherche, sélection, états de chargement et panneaux masquables. Persister le layout par identifiant de panneau et les raccourcis `G`, `E`, `D`, `P`, `X`, `1` à `5`, `Tab` via le contrat versionné. Ajouter `shell_gate.rs` pour mesurer le bridge, le focus, le chargement local et le smoke borné à 2 000 ms. Le gate des semaines 6-8 publie une décision. Si Tauri passe, ne pas produire ni activer `egui_shell`. Si Tauri échoue, geler le shell Tauri, activer `egui-shell`, implémenter uniquement `EguiShellModel`, `navigate_egui`, `render_egui_frame` et le binaire `src/bin/egui_shell.rs` avec les mêmes `AppCommand`/`AppEvent`; aucune seconde UI n’est développée en parallèle.

- [ ] **Step 4: Run test to verify it passes**

  Run: `npm ci --prefix crates/rawzelia-desktop/ui && npm test --prefix crates/rawzelia-desktop/ui -- --run src/library.test.ts src/lightroom_import.test.ts`

  Expected: PASS.

  Run from `crates/rawzelia-desktop`: `cargo test --test tauri_commands -- --exact desktop_command_rejects_unknown_resource`

  Expected: PASS.

  Run from `crates/rawzelia-desktop`: `cargo test --test lightroom_command_contract -- --exact desktop_lightroom_flow_uses_canonical_commands_through_append_artifact_and_rollback`

  Expected: PASS with one canonical `ReferenceJpegSelection` and fingerprint carried from inspect through execute/app mapping to `append_artifact`, then removed by the canonical rollback batch ID.

  Run from repository root: `cargo test -p rawzelia-desktop --test tauri_smoke -- --exact tauri_smoke_stays_local_and_bounded`

  Expected: PASS with `local_assets_only == true`, `focus_ok == true`, `library_smoke_ok == true` and `bridge_p95_ms <= 2000`.

  Run from `crates/rawzelia-desktop`: `cargo tauri build --target aarch64-apple-darwin`

  Expected: PASS with a local Apple Silicon bundle; this bounded build/smoke replaces toute vérification interactive non bornée.

  If and only if the Tauri gate is FAIL, run from repository root: `cargo check -p rawzelia-desktop --features egui-shell --bin egui_shell && cargo build -p rawzelia-desktop --features egui-shell --bin egui_shell && cargo test -p rawzelia-desktop --features egui-shell --test egui_smoke -- --exact egui_navigation_and_render_smoke`

  Expected: PASS with the optional egui shell reusing the same typed command/event contracts, navigation and a non-empty rendered frame; no ninth crate is added.

- [ ] **Step 5: Commit**

  ```bash
  git add crates/rawzelia-desktop
  git commit -m "feat: add tauri shell and library workflow"
  ```

### Task 9: Mode Développement, révisions, presets et synchronisation

**Files:**
- Modify: `crates/rawzelia-contract/src/commands.rs`
- Modify: `crates/rawzelia-app/src/use_cases.rs`
- Create: `crates/rawzelia-app/tests/edit_commands.rs`
- Create: `crates/rawzelia-desktop/ui/src/development.ts`
- Create: `crates/rawzelia-desktop/ui/src/development.test.ts`
- Modify: `crates/rawzelia-desktop/ui/src/main.ts`
- Modify: `crates/rawzelia-desktop/ui/src/styles.css`

**Interfaces:**
- Consumes: `EditDocument`, `Adjustment`, `RevisionId`, `PresetId`, `AppCommand`, `AppEvent`, `invokeCommand` et `subscribeToAppEvents`.
- Produces:
  - `pub fn append_adjustment(asset_id: AssetId, adjustment: Adjustment) -> Result<RevisionId, AppError>`;
  - `pub fn undo(asset_id: AssetId) -> Result<RevisionId, AppError>`;
  - `pub fn redo(asset_id: AssetId) -> Result<RevisionId, AppError>`;
  - `pub fn apply_preset(asset_ids: &[AssetId], preset_id: PresetId) -> Result<Vec<RevisionId>, AppError>`;
  - `pub fn copy_settings(source_asset_id: AssetId, target_asset_ids: &[AssetId], sections: &[SettingSection]) -> Result<Vec<RevisionId>, AppError>`;
  - `pub fn synchronize_settings(source_asset_id: AssetId, target_asset_ids: &[AssetId], sections: &[SettingSection]) -> Result<Vec<RevisionId>, AppError>`;
  - `export function buildSetAdjustmentCommand(assetId: string, adjustment: Adjustment): AppCommand`;
  - `export function buildCopySettingsCommand(sourceAssetId: string, targetAssetIds: string[], sections: SettingSection[]): AppCommand`;
  - `export function buildSynchronizeSettingsCommand(sourceAssetId: string, targetAssetIds: string[], sections: SettingSection[]): AppCommand`.

- [ ] **Step 1: Write the failing test**

  Ajouter `adjustment_is_immutable_and_survives_reopen`, `copy_settings_creates_one_revision_per_target` et `synchronize_settings_preserves_unselected_sections` dans `crates/rawzelia-app/tests/edit_commands.rs`. Ajouter dans `development.test.ts` les assertions sur les builders TypeScript et le dispatch unique par commande.

- [ ] **Step 2: Run test to verify it fails**

  Run: `cargo test -p rawzelia-app --test edit_commands -- --exact adjustment_is_immutable_and_survives_reopen`

  Expected: FAIL with `error[E0432]: unresolved import rawzelia_app::append_adjustment`.

- [ ] **Step 3: Implement the minimal Development workflow**

  Afficher image centrale, loupe, histogramme, filmstrip et réglages à droite. Appliquer exposition, contraste, hautes lumières, ombres, balance des blancs, crop, rotation, redressement et correction objectif ciblée. Chaque action produit une révision immuable et met à jour le sidecar. Undo/redo manipule les révisions ou commandes. Les presets et la copie/synchronisation sélectionnent explicitement les sections.

- [ ] **Step 4: Run test to verify it passes**

  Run: `cargo test -p rawzelia-app --test edit_commands && npm test --prefix crates/rawzelia-desktop/ui -- --run src/development.test.ts`

  Expected: PASS.

- [ ] **Step 5: Commit**

  ```bash
  git add crates/rawzelia-contract crates/rawzelia-app crates/rawzelia-desktop/ui
  git commit -m "feat: add non destructive development workflow"
  ```

### Task 10: Export JPEG/PNG et impression macOS

**Files:**
- Create: `crates/rawzelia-io/src/export.rs`
- Create: `crates/rawzelia-io/src/print.rs`
- Modify: `crates/rawzelia-io/src/lib.rs`
- Modify: `crates/rawzelia-desktop/src/commands.rs`
- Create: `crates/rawzelia-io/tests/export.rs`
- Create: `crates/rawzelia-io/tests/print.rs`

**Interfaces:**
- Consumes: `render_cpu`, `ExportCommand`, `PrintCommand`, `ExportProfile`, `AssetId`, `EditDocument`, `CatalogStore` et `AppCommand::Export` / `AppCommand::Print`.
- Produces:
  - `pub trait CpuExportRenderer: Send + Sync { fn render_export(&self, asset_id: AssetId, document: &EditDocument, profile: &ExportProfile, format: &ExportFormat) -> Result<RenderedImage, ExportError>; }`;
  - `pub fn export_batch(request: &ExportCommand, renderer: &dyn CpuExportRenderer, progress: &mut dyn FnMut(ExportProgress)) -> Result<ExportSummary, ExportError>`;
  - `pub struct ExportProgress { pub completed: u32, pub total: u32 }`, `pub struct ExportSummary { pub completed: u32, pub failed: u32, pub checksums: Vec<[u8; 32]> }`, `pub struct PrintJobId(pub Uuid);`;
  - `pub trait PrintBackend: Send + Sync { fn submit(&self, request: &PrintCommand, rendered: &[RenderedImage]) -> Result<PrintJobId, PrintError>; }`;
  - `pub fn print_selection(backend: &dyn PrintBackend, request: &PrintCommand, rendered: &[RenderedImage]) -> Result<PrintJobId, PrintError>`.

- [ ] **Step 1: Write the failing test**

  Ajouter `export_batch_is_deterministic_for_jpeg_full_and_web`, `png_embeds_selected_profile_when_supported` et `print_request_reaches_macos_backend`. Vérifier paramètres, profil, dimensions, checksum et collision de destination.

- [ ] **Step 2: Run test to verify it fails**

  Run: `cargo test -p rawzelia-io --test export -- --exact export_batch_is_deterministic_for_jpeg_full_and_web`

  Expected: FAIL with `error[E0432]: unresolved import rawzelia_io::export_batch`.

- [ ] **Step 3: Implement CPU export and system print boundary**

  Rendre JPEG pleine résolution, JPEG web et PNG via le CPU oracle. Enregistrer format, profil, qualité, dimension, version pipeline et checksum. Écrire temporairement puis renommer. Implémenter l’adaptateur macOS de `PrintBackend` dans `rawzelia-desktop`; garder la mise en page simple et le profil choisi explicite. Aucun export GPU ne doit être ajouté.

- [ ] **Step 4: Run test to verify it passes**

  Run: `cargo test -p rawzelia-io --test export && cargo test -p rawzelia-io --test print`

  Expected: PASS.

- [ ] **Step 5: Commit**

  ```bash
  git add crates/rawzelia-io crates/rawzelia-desktop
  git commit -m "feat: add cpu exports and macos print"
  ```

### Task 11: Intégration du flux v0.1, performance et gate semaine 8

**Files:**
- Create: `crates/rawzelia-app/tests/week_8_gate.rs`
- Create: `crates/rawzelia-app/tests/performance_gate.rs`
- Create: `crates/rawzelia-imaging/benches/v0_1.rs`
- Create: `.github/workflows/ci.yml`
- Modify: `crates/rawzelia-app/src/config.rs`
- Modify: `crates/rawzelia-desktop/tauri.conf.json`
- Modify: `crates/rawzelia-desktop/ui/package.json`

**Interfaces:**
- Consumes: tous les contrats, cas d’usage, import, stockage, rendu, UI et export des Tasks 1 à 10.
- Produces:
  - `pub fn run_week_8_flow(fixtures: &Path) -> Result<Week8Report, AppError>`;
  - `pub struct Week8Report { pub imported: u32, pub exported: u32, pub reopen_ok: bool, pub no_original_mutation: bool, pub performance: PerformanceReport }`;
  - `pub struct PerformanceReport { pub catalog_open_p95_ms: u64, pub search_p95_ms: u64, pub loupe_p95_ms: u64, pub proxy_adjustment_p95_ms: u64, pub memory_peak_bytes: u64, pub stale_render_count: u32, pub grid_fps: f32, pub pan_fps: f32, pub ui_block_p95_ms: u64, pub recovery_cycles: u32 }`;
  - `pub enum GateError { ImportCount, ExportCount, OriginalMutation, Reopen, Performance(String), Fixture(String), Tauri(String) }`, `pub fn assert_week_8_gate(report: &Week8Report) -> Result<(), GateError>` avec seuils de la spec.

- [ ] **Step 1: Write the failing test**

  Ajouter `d750_100_nef_round_trip` dans `week_8_gate.rs` et `week_8_performance_budgets` dans `performance_gate.rs`. Le test doit importer, trier, développer, persister, fermer, rouvrir et exporter 100 NEF D750. Il doit vérifier zéro mutation des originaux, 20 cycles fermeture/réouverture, 30 interruptions simulées réparties entre import, sidecar et sauvegarde, et les seuils p95.

- [ ] **Step 2: Run test to verify it fails**

  Run: `RAWZELIA_FIXTURES_DIR="$PWD/fixtures/rawzelia-v0.1" cargo test --release -p rawzelia-app --test week_8_gate -- --exact d750_100_nef_round_trip`

  Expected: FAIL with `error[E0432]: unresolved import rawzelia_app::run_week_8_flow` or a missing fixture/flow assertion.

- [ ] **Step 3: Implement the end-to-end gate and CI**

  Brancher le flux SD, catalogue, sidecar, CPU oracle, proxy/fallback, commandes Tauri et export. Préflighter l’unique `RAWZELIA_FIXTURES_DIR`, avec les NEF et golden sous `$RAWZELIA_FIXTURES_DIR/nef/` et les fixtures Lightroom sous `$RAWZELIA_FIXTURES_DIR/lightroom/`; la CI ne doit pas remplacer automatiquement les golden images. Ajouter les jobs CI pour `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets`, `npm ci --prefix crates/rawzelia-desktop/ui`, `npm run check --prefix crates/rawzelia-desktop/ui` et le smoke build Tauri Apple Silicon. Borner les jobs et le smoke à 15 minutes côté CI, sans commande interactive. Ajouter la mesure de mémoire, p95, stale renders, fps, blocage UI et reprise.

- [ ] **Step 4: Run test to verify it passes**

  Run from repository root: `cargo fmt --all -- --check`

  Expected: PASS.

  Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`

  Expected: PASS.

  Run: `cargo test --workspace --all-targets`

  Expected: PASS.

  Run: `npm ci --prefix crates/rawzelia-desktop/ui && npm run check --prefix crates/rawzelia-desktop/ui && npm test --prefix crates/rawzelia-desktop/ui -- --run`

  Expected: PASS.

  Run: `RAWZELIA_FIXTURES_DIR="$PWD/fixtures/rawzelia-v0.1" cargo test --release -p rawzelia-app --test week_8_gate -- --exact d750_100_nef_round_trip`

  Expected: PASS with `imported == 100`, `exported == 100`, `reopen_ok == true`, `no_original_mutation == true`, `stale_render_count == 0` and `recovery_cycles == 30`.

  Run from `crates/rawzelia-desktop`: `cargo tauri build --target aarch64-apple-darwin`

  Expected: PASS with an Apple Silicon Tauri bundle containing the local UI and no remote URL.

- [ ] **Step 5: Commit**

  ```bash
  git add crates/rawzelia-app crates/rawzelia-imaging crates/rawzelia-desktop .github/workflows/ci.yml
  git commit -m "test: enforce rawzelia week eight gate"
  ```

## Coverage audit

- Workspace Rust et huit crates physiques : Task 1.
- Contrats Community, Pro et Lightroom, namespaces, `LightroomSchemaId`, `ImportStatus`, `FieldProvenance`, `ImportedField<T>`, `UnknownField`, `InspectLightroomCommand`, `ExecuteLightroomCommand`, `ReferenceJpegSelection` et `RollbackLightroomCommand` canoniques : Task 2 ; le garde source est minimal et le trait/adaptateur complet reste au plan import.
- Catalogue SQLite, WAL, migrations, sauvegarde, sidecar `.rawzelia` et XMP miroir : Task 3.
- Volumes multiples, import SD, modèle de chemin, hash, déduplication et reprise : Task 4.
- NEF D750, LibRaw, CPU oracle, profils ICC, `DcpProfileRef`, les quatre objectifs, Samyang manuel/sans EXIF, droits de profil et golden images : Task 5.
- Previews, cache, tuiles, proxy `wgpu`, génération et fallback CPU : Task 6.
- Commandes/events typés, jobs, reprise, suppression `CatalogOnly`/`MoveToTrash`, `TrashBackend` et aucun REST : Task 7.
- Shell Tauri 2, Bibliothèque, filmstrip, panneaux, keymap, raccourcis et build Apple Silicon : Task 8.
- Développement, histogramme, réglages, presets, copy/sync et révisions : Task 9.
- JPEG pleine résolution, JPEG web, PNG, ICC et impression : Task 10.
- Performance, CI, golden, 100 NEF et gate semaine 8 : Task 11 ; toutes les fixtures passent par `RAWZELIA_FIXTURES_DIR`, avec `nef/` pour le corpus NEF et `lightroom/` pour Lightroom.
- Importeur Lightroom complet, adaptateurs de schéma antérieurs, Smart Previews et récupération Develop détaillée : hors implémentation de ce plan, contrat et garde source préparés au Task 2 ; suite obligatoire après le gate semaine 8 selon la spec.

## Definition of Done

- [ ] Les huit crates physiques existent exactement sous les noms de la spec, compilent et ne dépendent pas du dépôt Pro pour le parcours Community.
- [ ] `rawzelia-contract` expose commandes, événements, capacités, keymap, layout et ressources versionnés, utilisés par menu, bouton, palette et raccourcis.
- [ ] `rawzelia-contract/src/commands.rs` est l’unique propriétaire d’`AppCommand` et des commandes Lightroom ; `InspectLightroomCommand.reference_jpeg: Option<ReferenceJpegSelection>` et `ExecuteLightroomCommand.reference_jpeg: Option<ReferenceJpegSelection>` sont sérialisés, le bridge TypeScript porte le même `reference_jpeg?` et fingerprint, et la séquence sélection/inspect/execute/app/`append_artifact`/rollback est couverte entre contract, app et desktop sans redéfinition.
- [ ] `rawzelia-contract/src/lightroom_import.rs` est l’unique source de `LightroomSchemaId`, `ImportStatus`, `FieldProvenance`, `ImportedField<T>` et `UnknownField`; le test partagé compile core, import et hardening contre `source_schema: LightroomSchemaId`, et `serde_json::Value` n’existe que dans `UnknownField.raw_value`.
- [ ] L’application Tauri 2 se lance et se construit pour `aarch64-apple-darwin` sans URL distante.
- [ ] Le spike et le smoke Tauri des semaines 6-8 mesurent le bridge, le focus et les assets locaux dans 2 000 ms ; en cas d’échec, la décision gèle Tauri et active seulement le binaire `egui_shell` conditionnel du même crate.
- [ ] Si le gate Tauri échoue, `cargo check`, `cargo build` et le smoke `egui-shell` passent avec les mêmes `AppCommand`/`AppEvent`, la navigation Bibliothèque/Développement et un rendu non vide ; aucune neuvième crate ni seconde UI parallèle n’est ajoutée.
- [ ] Le flux Bibliothèque permet grille, loupe, filmstrip, recherche, collections, étoiles, drapeaux, couleurs, mots-clés et panneaux masquables.
- [ ] Le flux Développement permet les réglages v0.1, histogramme, presets, copier, synchroniser, undo/redo et réouverture fidèle.
- [ ] 100 NEF Nikon D750 sont importés depuis une carte ou un dossier simulé, vérifiés par taille et hash, puis catalogués sans doublon.
- [ ] Le modèle de chemin par défaut et le changement de volume sont testés sur plusieurs racines.
- [ ] `RAWZELIA_FIXTURES_DIR` est la seule variable de fixtures ; `tests/fixtures/nef/manifest.toml` résout le corpus sous `$RAWZELIA_FIXTURES_DIR/nef/` et `tests/fixtures/lightroom/manifest.toml` sous `$RAWZELIA_FIXTURES_DIR/lightroom/`, sans l’ancienne variable sans suffixe `_DIR`.
- [ ] Aucun test ne détecte d’écriture dans un NEF, un catalogue Lightroom, un `.lrdata`, un XMP source ou un original.
- [ ] Les sidecars `.rawzelia` et le miroir XMP sont atomiques, versionnés, relisibles et soumis à résolution explicite en cas de divergence.
- [ ] La suppression exige une confirmation et distingue `CatalogOnly` de `MoveToTrash` via `TrashBackend`; un échec de corbeille conserve l’entrée catalogue.
- [ ] Le CPU fournit le rendu de référence, l’ICC est enregistré, le GPU proxy est optionnel, le fallback CPU fonctionne et aucun rendu obsolète n’est publié.
- [ ] `StorageError` est l’unique enum de stockage, `Keymap`/`BackupReport`/`RestoreReport` gardent les signatures détaillées du core, et le hardening les modifie ou les étend sans redéfinition.
- [ ] `DcpProfileRef` et `LensProfileSelection` couvrent les quatre objectifs, le Samyang sans EXIF, la correction manuelle, les droits de profil et l’invalidation du cache.
- [ ] JPEG pleine résolution, JPEG web, PNG et impression simple produisent des résultats vérifiables avec paramètres, profils et checksums.
- [ ] Les tests de corruption, interruption, reprise, sauvegarde, restauration, doublon, volume absent et permissions passent.
- [ ] Les golden images des quatre objectifs de référence et les fixtures NEF D750 sont versionnées ; aucune régénération automatique ne masque une régression.
- [ ] Les budgets p95, mémoire, 50 fps grille, 60 fps zoom/pan et 30 cycles de reprise sont mesurés sur Apple Silicon M1, 16 Go.
- [ ] Le gate semaine 8 passe sur 100 NEF avec import, tri, développement, sidecar, export, fermeture/réouverture et zéro mutation originale.
- [ ] La CI exécute format, clippy, tests workspace, tests UI, tests golden contrôlés et smoke build Tauri.
- [ ] L’importeur Lightroom reste lecture seule, refuse les sources actives ou verrouillées, et ses contrats de statuts/provenance sont couverts ; son parseur complet reste explicitement planifié après semaine 8.
- [ ] Le core ne crée dans `rawzelia-lightroom-import` que le squelette et le garde source ; l’association du JPEG Lightroom, les adaptateurs et le trait complet restent dans le plan import.
- [ ] `keymap.rs`, `backup.rs` et `AppRuntime::recover_interrupted_jobs` ont chacun un propriétaire unique indiqué dans le ledger ; les extensions ne recréent pas ces responsabilités.
- [ ] Les limites v0.1, la frontière Community/Pro, les licences et les futures étapes commerciales sont documentées sans promesse de parité Adobe.
- [ ] Le plan est revu avec la spec, aucune étape ne reste sans commande de validation et aucun commit n’est exécuté pendant la rédaction du plan.
