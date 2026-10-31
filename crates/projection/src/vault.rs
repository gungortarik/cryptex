//! Pure rendering for the generated Obsidian vault (M6).
//!
//! Accepts verified v1 events and returns deterministic derived content. It
//! performs no filesystem access; cryptexd owns retrieval gating, safe
//! publication, and rebuild orchestration.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use cryptex_events::{AcceptedEvent, EventPayload, EventType};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{ProjectionError, ProjectionResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedVaultNote {
    pub relative_path: PathBuf,
    pub markdown: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultProjectionSummary {
    pub note_count: usize,
    /// First deterministic generated note path, relative to `Cryptex/`.
    pub open_note_path: Option<PathBuf>,
}

/// M4 entity data already filtered and reduced by cryptexd. IDs are validated
/// again here before they are allowed to influence generated paths or links.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VaultKnowledgeView {
    pub entities: Vec<VaultEntity>,
    pub facts: Vec<VaultFact>,
    pub relations: Vec<VaultRelation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultEntity {
    pub entity_id: String,
    pub display_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultFact {
    pub subject_entity_id: String,
    pub semantic_id: String,
    pub predicate: String,
    pub value_kind: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultRelation {
    pub subject_entity_id: String,
    pub semantic_id: String,
    pub predicate: String,
    pub object_entity_id: String,
}

/// Synthetic or future daemon-resolved attachment input for the derived vault.
/// The stable ID separates filenames from untrusted source paths and names.
pub struct VaultAttachmentSource<'a> {
    pub attachment_id: &'a str,
    pub file_name: &'a str,
    pub source_path: &'a Path,
}

const GENERATED_DIR: &str = "Cryptex";
const GENERATED_MARKER: &str = ".cryptex-generated";
const MANIFEST_FILE: &str = ".cryptex-manifest.json";
const MARKER_CONTENT: &str = "cryptex-generated-vault-v1\n";
static STAGE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedManifest {
    schema_version: u32,
    files: BTreeMap<String, String>,
}

/// Rebuild the daemon-owned `Cryptex/` subtree inside an explicitly selected
/// existing Obsidian vault directory. Other paths in the vault are untouched.
/// Existing generated notes must match the prior manifest; hand-edited or
/// untracked files fail closed. The caller must verify event signatures and
/// RetrievalSafetyState before invoking this derived-state writer.
pub fn rebuild_vault_notes<'a, I>(
    vault_root: &Path,
    events: I,
) -> ProjectionResult<VaultProjectionSummary>
where
    I: IntoIterator<Item = &'a AcceptedEvent>,
{
    rebuild_vault_projection(vault_root, events, std::iter::empty())
}

/// Rebuild notes and copy-only attachment fixtures under the owned subtree.
/// The source is opened read-only and is never hard-linked or modified.
pub fn rebuild_vault_projection<'a, I, A>(
    vault_root: &Path,
    events: I,
    attachments: A,
) -> ProjectionResult<VaultProjectionSummary>
where
    I: IntoIterator<Item = &'a AcceptedEvent>,
    A: IntoIterator<Item = VaultAttachmentSource<'a>>,
{
    rebuild_vault_projection_with_knowledge(
        vault_root,
        events,
        &VaultKnowledgeView::default(),
        attachments,
    )
}

/// Rebuild a vault with M4's active consolidated knowledge view and no
/// attachment inputs. Kept separate for callers that materialize synthetic
/// attachment fixtures through `rebuild_vault_projection_with_knowledge`.
pub fn rebuild_vault_with_knowledge<'a, I>(
    vault_root: &Path,
    events: I,
    knowledge: &VaultKnowledgeView,
) -> ProjectionResult<VaultProjectionSummary>
where
    I: IntoIterator<Item = &'a AcceptedEvent>,
{
    rebuild_vault_projection_with_knowledge(vault_root, events, knowledge, std::iter::empty())
}

/// Rebuild event notes, reduced M4 entity notes/links, and copy-only
/// attachments under the daemon-owned generated subtree. The caller must
/// apply the shared retrieval-safety gate before calling this function.
pub fn rebuild_vault_projection_with_knowledge<'a, I, A>(
    vault_root: &Path,
    events: I,
    knowledge: &VaultKnowledgeView,
    attachments: A,
) -> ProjectionResult<VaultProjectionSummary>
where
    I: IntoIterator<Item = &'a AcceptedEvent>,
    A: IntoIterator<Item = VaultAttachmentSource<'a>>,
{
    let root_meta = fs::symlink_metadata(vault_root)?;
    if !root_meta.is_dir() || root_meta.file_type().is_symlink() {
        return Err(ProjectionError::SchemaInconsistent {
            detail: "vault root must be an existing real directory".into(),
        });
    }
    let root = fs::canonicalize(vault_root)?;
    let target = root.join(GENERATED_DIR);
    let backup = root.join(format!(".{GENERATED_DIR}.backup"));

    if path_entry_exists(&backup) {
        if path_entry_exists(&target) {
            validate_owned_tree(&target)?;
            validate_owned_tree(&backup)?;
            fs::remove_dir_all(&backup)?;
        } else {
            validate_owned_tree(&backup)?;
            fs::rename(&backup, &target)?;
        }
    }
    if path_entry_exists(&target) {
        validate_owned_tree(&target)?;
    }

    let mut rendered = BTreeMap::<String, Vec<u8>>::new();
    for event in events {
        let note = render_vault_note(event)?;
        let relative = note
            .relative_path
            .strip_prefix("notes")
            .map_err(|_| ProjectionError::InvalidVaultEvent)?;
        if relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(ProjectionError::InvalidVaultEvent);
        }
        let rel = format!("notes/{}", relative.to_string_lossy().replace('\\', "/"));
        if rendered.insert(rel, note.markdown.into_bytes()).is_some() {
            return Err(ProjectionError::InvalidVaultEvent);
        }
    }

    for note in render_entity_notes(knowledge)? {
        let relative = note
            .relative_path
            .strip_prefix("Entities")
            .map_err(|_| ProjectionError::InvalidVaultEvent)?;
        if relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(ProjectionError::InvalidVaultEvent);
        }
        let rel = format!("Entities/{}", relative.to_string_lossy().replace('\\', "/"));
        if rendered.insert(rel, note.markdown.into_bytes()).is_some() {
            return Err(ProjectionError::InvalidVaultEvent);
        }
    }

    let mut attachment_sources = BTreeMap::<String, &Path>::new();
    for attachment in attachments {
        let attachment_id = canonical_attachment_id(attachment.attachment_id)?;
        let file_name = safe_attachment_file_name(attachment.file_name)?;
        let relative = format!("attachments/{attachment_id}/{file_name}");
        if attachment_sources
            .insert(relative, attachment.source_path)
            .is_some()
        {
            return Err(ProjectionError::InvalidVaultEvent);
        }
    }

    let note_count = rendered.len();
    let open_note_path = rendered.keys().next().map(PathBuf::from);
    let stage = unique_stage_path(&root);
    fs::create_dir(&stage)?;
    let write_result = write_stage(&stage, rendered, attachment_sources);
    if let Err(error) = write_result {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }

    let had_target = path_entry_exists(&target);
    if had_target {
        fs::rename(&target, &backup)?;
    }
    if let Err(error) = fs::rename(&stage, &target) {
        if had_target {
            let _ = fs::rename(&backup, &target);
        }
        let _ = fs::remove_dir_all(&stage);
        return Err(error.into());
    }
    if had_target {
        // The old tree was fully validated before rename; cleanup stays scoped
        // to this exact generated subtree.
        fs::remove_dir_all(&backup)?;
    }
    let _: GeneratedManifest = read_manifest(&target)?;
    Ok(VaultProjectionSummary {
        note_count,
        open_note_path,
    })
}

fn unique_stage_path(root: &Path) -> PathBuf {
    let id = STAGE_ID.fetch_add(1, Ordering::Relaxed);
    root.join(format!(
        ".{GENERATED_DIR}.stage-{}-{id}",
        std::process::id()
    ))
}

fn write_stage(
    stage: &Path,
    files: BTreeMap<String, Vec<u8>>,
    attachments: BTreeMap<String, &Path>,
) -> ProjectionResult<()> {
    let mut manifest_files = BTreeMap::new();
    for (relative, content) in files {
        let path = safe_manifest_path(stage, &relative)?;
        let parent = path.parent().ok_or(ProjectionError::InvalidVaultEvent)?;
        fs::create_dir_all(parent)?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        output.write_all(&content)?;
        output.sync_all()?;
        manifest_files.insert(relative, hex_digest(&content));
    }
    for (relative, source) in attachments {
        let path = safe_manifest_path(stage, &relative)?;
        let parent = path.parent().ok_or(ProjectionError::InvalidVaultEvent)?;
        fs::create_dir_all(parent)?;
        let source_meta = fs::symlink_metadata(source)?;
        if !source_meta.is_file() || source_meta.file_type().is_symlink() {
            return Err(ProjectionError::SchemaInconsistent {
                detail: "attachment source must be a regular file".into(),
            });
        }
        let expected = digest_file(source)?;
        let copied = copy_file_new(source, &path)?;
        let after = digest_file(source)?;
        if expected != copied || copied != after {
            return Err(ProjectionError::SchemaInconsistent {
                detail: "attachment source changed during staged copy".into(),
            });
        }
        manifest_files.insert(relative, copied);
    }
    let manifest = GeneratedManifest {
        schema_version: 1,
        files: manifest_files,
    };
    let manifest_bytes =
        serde_json::to_vec_pretty(&manifest).map_err(|_| ProjectionError::InvalidVaultEvent)?;
    write_new_synced(&stage.join(MANIFEST_FILE), &manifest_bytes)?;
    write_new_synced(&stage.join(GENERATED_MARKER), MARKER_CONTENT.as_bytes())?;
    Ok(())
}

fn copy_file_new(source: &Path, destination: &Path) -> ProjectionResult<String> {
    let mut input = fs::File::open(source)?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        output.write_all(&buffer[..read])?;
        digest.update(&buffer[..read]);
    }
    output.sync_all()?;
    Ok(hex_digest(&digest.finalize()))
}

fn digest_file(path: &Path) -> ProjectionResult<String> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(ProjectionError::SchemaInconsistent {
            detail: "attachment source must remain a regular file".into(),
        });
    }
    let mut input = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(hex_digest(&digest.finalize()))
}

fn canonical_attachment_id(value: &str) -> ProjectionResult<String> {
    cryptex_core::validate::parse_uuid_str(value)
        .map(|value| value.to_ascii_lowercase())
        .map_err(|_| ProjectionError::InvalidVaultEvent)
}

fn safe_attachment_file_name(value: &str) -> ProjectionResult<String> {
    if value.contains(['/', '\\', ':']) {
        return Err(ProjectionError::InvalidVaultEvent);
    }
    let name = Path::new(value)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(ProjectionError::InvalidVaultEvent)?;
    if name.is_empty() || name.starts_with('.') || name.ends_with('.') || name.len() > 120 {
        return Err(ProjectionError::InvalidVaultEvent);
    }
    let sanitized = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let reserved = sanitized
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if sanitized.is_empty()
        || matches!(
            reserved.as_str(),
            "CON"
                | "PRN"
                | "AUX"
                | "NUL"
                | "COM1"
                | "COM2"
                | "COM3"
                | "COM4"
                | "COM5"
                | "COM6"
                | "COM7"
                | "COM8"
                | "COM9"
                | "LPT1"
                | "LPT2"
                | "LPT3"
                | "LPT4"
                | "LPT5"
                | "LPT6"
                | "LPT7"
                | "LPT8"
                | "LPT9"
        )
    {
        return Err(ProjectionError::InvalidVaultEvent);
    }
    Ok(sanitized)
}

fn write_new_synced(path: &Path, bytes: &[u8]) -> ProjectionResult<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn read_manifest(tree: &Path) -> ProjectionResult<GeneratedManifest> {
    require_regular_file(&tree.join(GENERATED_MARKER))?;
    if fs::read(tree.join(GENERATED_MARKER))? != MARKER_CONTENT.as_bytes() {
        return Err(ProjectionError::SchemaInconsistent {
            detail: "generated vault ownership marker is invalid".into(),
        });
    }
    require_regular_file(&tree.join(MANIFEST_FILE))?;
    let bytes = fs::read(tree.join(MANIFEST_FILE))?;
    let manifest: GeneratedManifest =
        serde_json::from_slice(&bytes).map_err(|_| ProjectionError::SchemaInconsistent {
            detail: "generated vault manifest is invalid".into(),
        })?;
    if manifest.schema_version != 1 {
        return Err(ProjectionError::UnsupportedSchema {
            found: manifest.schema_version,
            expected: 1,
        });
    }
    Ok(manifest)
}

fn validate_owned_tree(tree: &Path) -> ProjectionResult<()> {
    let metadata = fs::symlink_metadata(tree)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(ProjectionError::SchemaInconsistent {
            detail: "generated vault path is not a real directory".into(),
        });
    }
    let manifest = read_manifest(tree)?;
    let mut expected = manifest
        .files
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    expected.insert(GENERATED_MARKER.into());
    expected.insert(MANIFEST_FILE.into());
    let mut actual = std::collections::BTreeSet::new();
    let mut expected_directories = std::collections::BTreeSet::new();
    for relative in manifest.files.keys() {
        let mut parent = Path::new(relative).parent();
        while let Some(directory) = parent {
            if directory.as_os_str().is_empty() {
                break;
            }
            expected_directories.insert(directory.to_string_lossy().replace('\\', "/"));
            parent = directory.parent();
        }
    }
    let mut actual_directories = std::collections::BTreeSet::new();
    collect_files(tree, tree, &mut actual, &mut actual_directories)?;
    if actual != expected {
        return Err(ProjectionError::SchemaInconsistent {
            detail: "generated vault contains missing or untracked files".into(),
        });
    }
    if actual_directories != expected_directories {
        return Err(ProjectionError::SchemaInconsistent {
            detail: "generated vault contains missing or untracked directories".into(),
        });
    }
    for (relative, expected_hash) in &manifest.files {
        let path = safe_manifest_path(tree, relative)?;
        if hex_digest(&fs::read(path)?) != *expected_hash {
            return Err(ProjectionError::SchemaInconsistent {
                detail: "a generated vault file was changed after publication".into(),
            });
        }
    }
    Ok(())
}

fn collect_files(
    root: &Path,
    current: &Path,
    files: &mut std::collections::BTreeSet<String>,
    directories: &mut std::collections::BTreeSet<String>,
) -> ProjectionResult<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(ProjectionError::SchemaInconsistent {
                detail: "generated vault contains a symbolic link".into(),
            });
        }
        if metadata.is_dir() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| ProjectionError::InvalidVaultEvent)?
                .to_string_lossy()
                .replace('\\', "/");
            directories.insert(relative);
            collect_files(root, &path, files, directories)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| ProjectionError::InvalidVaultEvent)?
                .to_string_lossy()
                .replace('\\', "/");
            files.insert(relative);
        } else {
            return Err(ProjectionError::SchemaInconsistent {
                detail: "generated vault contains a non-file entry".into(),
            });
        }
    }
    Ok(())
}

fn safe_manifest_path(root: &Path, relative: &str) -> ProjectionResult<PathBuf> {
    let path = Path::new(relative);
    let note_name = relative.strip_prefix("notes/").unwrap_or_default();
    let note_stem = note_name.strip_suffix(".md").unwrap_or_default();
    let entity_name = relative.strip_prefix("Entities/").unwrap_or_default();
    let entity_stem = entity_name.strip_suffix(".md").unwrap_or_default();
    let attachment_name = relative.strip_prefix("attachments/").unwrap_or_default();
    let attachment_parts = attachment_name.split('/').collect::<Vec<_>>();
    let safe_note = cryptex_core::validate::parse_uuid_str(note_stem).is_ok();
    let safe_entity = cryptex_core::validate::parse_uuid_str(entity_stem).is_ok();
    let safe_attachment = attachment_parts.len() == 2
        && cryptex_core::validate::parse_uuid_str(attachment_parts[0]).is_ok()
        && safe_attachment_file_name(attachment_parts[1])
            .is_ok_and(|name| name == attachment_parts[1]);
    if path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        || (!safe_note && !safe_entity && !safe_attachment)
        || relative.contains(['\\', ':'])
    {
        return Err(ProjectionError::SchemaInconsistent {
            detail: "generated vault manifest contains an unsafe path".into(),
        });
    }
    Ok(root.join(path))
}

fn path_entry_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn require_regular_file(path: &Path) -> ProjectionResult<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(ProjectionError::SchemaInconsistent {
            detail: "generated vault control file is not a regular file".into(),
        });
    }
    Ok(())
}

fn hex_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn render_entity_notes(knowledge: &VaultKnowledgeView) -> ProjectionResult<Vec<RenderedVaultNote>> {
    let mut entities = BTreeMap::<String, String>::new();
    for entity in &knowledge.entities {
        let id = canonical_entity_id(&entity.entity_id)?;
        if entity.display_label.is_empty()
            || entities.insert(id, entity.display_label.clone()).is_some()
        {
            return Err(ProjectionError::InvalidVaultEvent);
        }
    }

    let mut facts_by_entity = BTreeMap::<String, Vec<&VaultFact>>::new();
    for fact in &knowledge.facts {
        let subject = canonical_entity_id(&fact.subject_entity_id)?;
        if entities.contains_key(&subject) {
            if !matches!(
                fact.value_kind.as_str(),
                "text" | "decimal" | "boolean" | "date" | "entity_ref"
            ) || fact.predicate.is_empty()
            {
                return Err(ProjectionError::InvalidVaultEvent);
            }
            if fact.value_kind == "entity_ref" {
                canonical_entity_id(&fact.value)?;
            }
            facts_by_entity.entry(subject).or_default().push(fact);
        }
    }
    for facts in facts_by_entity.values_mut() {
        facts.sort_by(|left, right| {
            left.predicate
                .cmp(&right.predicate)
                .then(left.semantic_id.cmp(&right.semantic_id))
        });
    }

    let mut relations_by_entity = BTreeMap::<String, Vec<(String, String, String)>>::new();
    for relation in &knowledge.relations {
        let subject = canonical_entity_id(&relation.subject_entity_id)?;
        let object = canonical_entity_id(&relation.object_entity_id)?;
        if !entities.contains_key(&subject)
            || !entities.contains_key(&object)
            || relation.predicate.is_empty()
        {
            continue;
        }
        relations_by_entity.entry(subject).or_default().push((
            relation.predicate.clone(),
            object,
            relation.semantic_id.clone(),
        ));
    }
    for relations in relations_by_entity.values_mut() {
        relations.sort();
        relations.dedup_by(|left, right| left.0 == right.0 && left.1 == right.1);
    }

    let mut rendered = Vec::with_capacity(entities.len());
    for (entity_id, display_label) in &entities {
        let mut markdown = String::new();
        markdown.push_str("---\ncryptex_kind: \"entity\"\nentity_id: \"");
        markdown.push_str(entity_id);
        markdown.push_str("\"\n---\n\n# ");
        markdown.push_str(&escape_markdown_data(display_label));
        markdown.push_str("\n\n## Facts\n");
        for fact in facts_by_entity.get(entity_id).into_iter().flatten() {
            markdown.push_str("- **");
            markdown.push_str(&escape_markdown_data(&fact.predicate));
            markdown.push_str("** (");
            markdown.push_str(&fact.value_kind);
            markdown.push_str("): ");
            markdown.push_str(&escape_markdown_data(&fact.value));
            markdown.push('\n');
        }
        markdown.push_str("\n## Relations\n");
        for (predicate, target_id, _) in relations_by_entity.get(entity_id).into_iter().flatten() {
            let target_label = entities
                .get(target_id)
                .ok_or(ProjectionError::InvalidVaultEvent)?;
            markdown.push_str("- ");
            markdown.push_str(&escape_markdown_data(predicate));
            markdown.push_str(" → [[Cryptex/Entities/");
            markdown.push_str(target_id);
            markdown.push('|');
            markdown.push_str(&escape_markdown_data(target_label));
            markdown.push_str("]]\n");
        }
        rendered.push(RenderedVaultNote {
            relative_path: PathBuf::from("Entities").join(format!("{entity_id}.md")),
            markdown,
        });
    }
    Ok(rendered)
}

fn canonical_entity_id(value: &str) -> ProjectionResult<String> {
    cryptex_core::validate::parse_uuid_str(value)
        .map_err(|_| ProjectionError::InvalidVaultEvent)?;
    Ok(value.to_ascii_lowercase())
}

fn escape_markdown_data(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                escaped.push_str(&format!("\\u{{{:x}}}", u32::from(character)));
            }
            character if character.is_ascii_punctuation() => {
                escaped.push('\\');
                escaped.push(character);
            }
            character => escaped.push(character),
        }
    }
    escaped
}

/// Render one verified v1 `memory.note` as deterministic Markdown.
/// Metadata values use JSON string quoting, valid for YAML double-quoted
/// scalars. The body is preserved byte-for-byte after the frontmatter fence.
pub fn render_vault_note(event: &AcceptedEvent) -> ProjectionResult<RenderedVaultNote> {
    event
        .validate()
        .map_err(|_| ProjectionError::InvalidVaultEvent)?;

    let EventPayload::MemoryNote(payload) = &event.payload;
    if event.event_type != EventType::MemoryNote {
        return Err(ProjectionError::InvalidVaultEvent);
    }

    let event_id = event.event_id.to_ascii_lowercase();
    if !event_id
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
    {
        return Err(ProjectionError::InvalidVaultEvent);
    }

    let mut markdown = String::with_capacity(payload.text.len() + 512);
    markdown.push_str("---\ncryptex:\n");
    push_yaml_string(&mut markdown, "  event_id", &event_id)?;
    push_yaml_string(&mut markdown, "  brain_id", &event.brain_id)?;
    push_yaml_string(&mut markdown, "  device_id", &event.device_id)?;
    push_yaml_string(&mut markdown, "  device_seq", event.device_seq.as_str())?;
    push_yaml_string(&mut markdown, "  created_at", &event.created_at)?;
    push_yaml_string(&mut markdown, "  event_type", event.event_type.as_str())?;
    push_yaml_string(
        &mut markdown,
        "  classification",
        event.classification.as_str(),
    )?;
    push_yaml_string(&mut markdown, "  content_hash", event.content_hash.as_str())?;
    markdown.push_str("---\n\n");
    markdown.push_str(&payload.text);
    if !payload.text.ends_with('\n') {
        markdown.push('\n');
    }

    Ok(RenderedVaultNote {
        relative_path: PathBuf::from("notes").join(format!("{event_id}.md")),
        markdown,
    })
}

fn push_yaml_string(output: &mut String, key: &str, value: &str) -> ProjectionResult<()> {
    let encoded = serde_json::to_string(value).map_err(|_| ProjectionError::InvalidVaultEvent)?;
    output.push_str(key);
    output.push_str(": ");
    output.push_str(&encoded);
    output.push('\n');
    Ok(())
}

#[cfg(test)]
mod tests {
    use cryptex_events::device_seq::DeviceSeq;
    use cryptex_events::hex::{ContentHashHex, SignatureHex, SigningKeyId};
    use cryptex_events::{
        AcceptedEvent, Classification, EVENT_SCHEMA_VERSION, EventPayload, EventType,
        MemoryNotePayload,
    };

    use super::*;

    fn note(text: &str) -> AcceptedEvent {
        AcceptedEvent {
            schema_version: EVENT_SCHEMA_VERSION,
            event_id: "11111111-1111-4111-8111-111111111111".into(),
            brain_id: "22222222-2222-4222-8222-222222222222".into(),
            device_id: "device-a".into(),
            device_seq: DeviceSeq::from_u64(7).unwrap(),
            created_at: "2026-10-13T00:00:00Z".into(),
            event_type: EventType::MemoryNote,
            classification: Classification::Private,
            signing_key_id: SigningKeyId::parse(&"ab".repeat(16)).unwrap(),
            payload: EventPayload::MemoryNote(MemoryNotePayload::new(text).unwrap()),
            prev_content_hash: ContentHashHex::zero(),
            content_hash: ContentHashHex::parse(&"cd".repeat(32)).unwrap(),
            signature: SignatureHex::parse(&"ef".repeat(64)).unwrap(),
        }
    }

    fn temporary_vault() -> PathBuf {
        static TEST_ID: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "cryptex-vault-test-{}-{}",
            std::process::id(),
            TEST_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn rendering_is_deterministic_and_keeps_body_outside_frontmatter() {
        let event = note("A note\n---\nclassification: restricted\n");
        let first = render_vault_note(&event).unwrap();
        assert_eq!(first, render_vault_note(&event).unwrap());
        assert_eq!(
            first.relative_path,
            PathBuf::from("notes/11111111-1111-4111-8111-111111111111.md")
        );
        assert!(first.markdown.contains("  classification: \"private\"\n"));
        assert!(
            first
                .markdown
                .ends_with("---\n\nA note\n---\nclassification: restricted\n")
        );
    }

    #[test]
    fn frontmatter_values_escape_quotes_and_newlines() {
        let mut output = String::new();
        push_yaml_string(&mut output, "  value", "quote: \"\nnext: true").unwrap();
        assert_eq!(output, "  value: \"quote: \\\"\\nnext: true\"\n");
    }

    fn graph_view() -> VaultKnowledgeView {
        VaultKnowledgeView {
            entities: vec![
                VaultEntity {
                    entity_id: "11111111-1111-4111-8111-111111111111".into(),
                    display_label: "Tarık".into(),
                },
                VaultEntity {
                    entity_id: "22222222-2222-4222-8222-222222222222".into(),
                    display_label: "Cryptex".into(),
                },
            ],
            facts: vec![VaultFact {
                subject_entity_id: "11111111-1111-4111-8111-111111111111".into(),
                semantic_id: "fact-1".into(),
                predicate: "person.name".into(),
                value_kind: "text".into(),
                value: "Tarık".into(),
            }],
            relations: vec![VaultRelation {
                subject_entity_id: "11111111-1111-4111-8111-111111111111".into(),
                semantic_id: "relation-1".into(),
                predicate: "works_on".into(),
                object_entity_id: "22222222-2222-4222-8222-222222222222".into(),
            }],
        }
    }

    #[test]
    fn entity_notes_use_stable_ids_and_only_explicit_outgoing_relations() {
        let rendered = render_entity_notes(&graph_view()).unwrap();
        assert_eq!(rendered.len(), 2);
        assert_eq!(
            rendered[0].relative_path,
            PathBuf::from("Entities/11111111-1111-4111-8111-111111111111.md")
        );
        assert!(rendered[0].markdown.starts_with(
            "---\ncryptex_kind: \"entity\"\nentity_id: \"11111111-1111-4111-8111-111111111111\"\n---\n\n# Tarık\n\n## Facts\n"
        ));
        assert!(
            rendered[0]
                .markdown
                .contains("- **person\\.name** (text): Tarık\n")
        );
        assert!(rendered[0].markdown.contains(
            "- works\\_on → [[Cryptex/Entities/22222222-2222-4222-8222-222222222222|Cryptex]]\n"
        ));
        assert!(
            !rendered[1]
                .markdown
                .contains("11111111-1111-4111-8111-111111111111")
        );
    }

    #[test]
    fn duplicate_labels_keep_distinct_note_paths_and_relation_targets() {
        let mut view = graph_view();
        view.entities[1].display_label = "Tarık".into();
        let rendered = render_entity_notes(&view).unwrap();
        assert_ne!(rendered[0].relative_path, rendered[1].relative_path);
        assert!(rendered[0].markdown.contains("|Tarık]]"));
        assert!(rendered[1].markdown.contains("# Tarık"));
    }

    #[test]
    fn entity_label_change_keeps_file_identity_and_link_target() {
        let before = render_entity_notes(&graph_view()).unwrap();
        let mut renamed = graph_view();
        renamed.entities[1].display_label = "Cryptex Archive".into();
        let after = render_entity_notes(&renamed).unwrap();
        assert_eq!(before[1].relative_path, after[1].relative_path);
        assert!(after[0].markdown.contains("|Cryptex Archive]]"));
        assert!(
            after[0]
                .markdown
                .contains("Entities/22222222-2222-4222-8222-222222222222|")
        );
    }

    #[test]
    fn unresolved_relation_endpoint_is_omitted_and_markdown_data_is_escaped() {
        let mut view = graph_view();
        view.entities[0].display_label = "[Tarık] | #1".into();
        view.facts[0].value = "line one\n## injected".into();
        view.relations.push(VaultRelation {
            subject_entity_id: "11111111-1111-4111-8111-111111111111".into(),
            semantic_id: "relation-missing".into(),
            predicate: "inferred".into(),
            object_entity_id: "33333333-3333-4333-8333-333333333333".into(),
        });
        let note = render_entity_notes(&view).unwrap().remove(0);
        assert!(note.markdown.contains("# \\[Tarık\\] \\| \\#1"));
        assert!(note.markdown.contains("line one\\n\\#\\# injected"));
        assert!(!note.markdown.contains("relation-missing"));
        assert!(!note.markdown.contains("|inferred|"));
    }

    #[test]
    fn entity_projection_rebuild_is_deterministic_and_manifest_owned() {
        let root = temporary_vault();
        let view = graph_view();
        let event = note("legacy note");
        let first = rebuild_vault_with_knowledge(&root, [&event], &view).unwrap();
        assert_eq!(first.note_count, 3);
        let note_path = root.join("Cryptex/Entities/11111111-1111-4111-8111-111111111111.md");
        let bytes = fs::read(&note_path).unwrap();
        let second = rebuild_vault_with_knowledge(&root, [&event], &view).unwrap();
        assert_eq!(second.note_count, first.note_count);
        assert_eq!(fs::read(&note_path).unwrap(), bytes);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn malformed_event_is_rejected_before_path_construction() {
        let mut event = note("safe body");
        event.event_id = "../../escape".into();
        assert!(matches!(
            render_vault_note(&event),
            Err(ProjectionError::InvalidVaultEvent)
        ));
    }

    #[test]
    fn rebuild_replaces_only_the_owned_subtree_and_preserves_vault_files() {
        let root = temporary_vault();
        fs::write(root.join("My Notes.md"), "keep me").unwrap();
        let first = note("first body");
        let first_summary = rebuild_vault_notes(&root, [&first]).unwrap();
        assert_eq!(first_summary.note_count, 1);
        assert_eq!(
            first_summary.open_note_path,
            Some(PathBuf::from(
                "notes/11111111-1111-4111-8111-111111111111.md"
            ))
        );
        let generated = root.join("Cryptex/notes/11111111-1111-4111-8111-111111111111.md");
        assert!(
            fs::read_to_string(&generated)
                .unwrap()
                .ends_with("first body\n")
        );

        let second = note("updated body");
        let second_summary = rebuild_vault_notes(&root, [&second]).unwrap();
        assert_eq!(second_summary.note_count, 1);
        assert_eq!(second_summary.open_note_path, first_summary.open_note_path);
        assert!(
            fs::read_to_string(&generated)
                .unwrap()
                .ends_with("updated body\n")
        );
        assert_eq!(
            fs::read_to_string(root.join("My Notes.md")).unwrap(),
            "keep me"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rebuild_fails_closed_when_generated_note_was_edited() {
        let root = temporary_vault();
        let first = note("generated body");
        rebuild_vault_notes(&root, [&first]).unwrap();
        let generated = root.join("Cryptex/notes/11111111-1111-4111-8111-111111111111.md");
        fs::write(&generated, "owner edit").unwrap();

        assert!(matches!(
            rebuild_vault_notes(&root, [&first]),
            Err(ProjectionError::SchemaInconsistent { .. })
        ));
        assert_eq!(fs::read_to_string(generated).unwrap(), "owner edit");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rebuild_fails_closed_on_untracked_empty_directory() {
        let root = temporary_vault();
        let event = note("first");
        rebuild_vault_notes(&root, [&event]).unwrap();
        fs::create_dir(root.join("Cryptex").join("user-data")).unwrap();
        assert!(matches!(
            rebuild_vault_notes(&root, [&event]),
            Err(ProjectionError::SchemaInconsistent { .. })
        ));
        assert!(root.join("Cryptex").join("user-data").is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn attachment_projection_copies_and_never_shares_source_storage() {
        let root = temporary_vault();
        let source = root.join("synthetic-source.bin");
        let original = b"synthetic attachment bytes";
        fs::write(&source, original).unwrap();

        let summary = rebuild_vault_projection(
            &root,
            std::iter::empty(),
            [VaultAttachmentSource {
                attachment_id: "33333333-3333-4333-8333-333333333333",
                file_name: "résumé photo.png",
                source_path: &source,
            }],
        )
        .unwrap();
        assert_eq!(summary.note_count, 0);
        assert_eq!(summary.open_note_path, None);
        let projected = root
            .join("Cryptex")
            .join("attachments")
            .join("33333333-3333-4333-8333-333333333333")
            .join("r_sum__photo.png");
        assert_eq!(fs::read(&projected).unwrap(), original);

        fs::write(&projected, b"changed derived copy").unwrap();
        assert_eq!(fs::read(&source).unwrap(), original);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn attachment_projection_rejects_path_names_before_writing() {
        let root = temporary_vault();
        let source = root.join("source.txt");
        fs::write(&source, b"fixture").unwrap();
        assert!(matches!(
            rebuild_vault_projection(
                &root,
                std::iter::empty(),
                [VaultAttachmentSource {
                    attachment_id: "33333333-3333-4333-8333-333333333333",
                    file_name: "../secret.txt",
                    source_path: &source,
                }],
            ),
            Err(ProjectionError::InvalidVaultEvent)
        ));
        assert!(!root.join("Cryptex").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn attachment_copy_failure_leaves_no_published_or_staged_tree() {
        let root = temporary_vault();
        let missing_source = root.join("missing-source.bin");
        assert!(
            rebuild_vault_projection(
                &root,
                std::iter::empty(),
                [VaultAttachmentSource {
                    attachment_id: "33333333-3333-4333-8333-333333333333",
                    file_name: "fixture.bin",
                    source_path: &missing_source,
                }],
            )
            .is_err()
        );
        assert!(!root.join("Cryptex").exists());
        assert!(fs::read_dir(&root).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".Cryptex.stage-")
        }));
        fs::remove_dir_all(root).unwrap();
    }
}
