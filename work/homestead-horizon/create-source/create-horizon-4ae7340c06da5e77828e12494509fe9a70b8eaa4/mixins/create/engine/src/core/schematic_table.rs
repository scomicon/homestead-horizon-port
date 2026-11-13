// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Pure Schematic Table and upload rules after SchematicTableBlockEntity,
// SchematicTableMenu, ServerSchematicLoader, ClientSchematicLoader and
// CSchematics (Create 6.0.10). File IO, networking and the player's schematic
// folder belong to adapters; this module decides what is accepted and in
// which order, and tracks upload session state.
use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::schematic_tool::{EMPTY_SCHEMATIC_ITEM, SCHEMATIC_AND_QUILL_ITEM, SCHEMATIC_ITEM};

/// `CSchematics` defaults (server config).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UploadConfig {
    /// Files kept per player before the oldest is replaced (min 1).
    pub max_schematics: usize,
    /// Largest accepted upload in KB (min 16); compared as `kb * 1000` bytes.
    pub max_total_schematic_size_kb: u64,
    /// Largest packet payload in bytes (256..=32767).
    pub max_schematic_packet_size: usize,
    /// Ticks without packets before an upload is dropped (min 100).
    pub schematic_idle_timeout: i32,
}

impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            max_schematics: 10,
            max_total_schematic_size_kb: 256,
            max_schematic_packet_size: 1024,
            schematic_idle_timeout: 600,
        }
    }
}

impl UploadConfig {
    /// Clamp to the ranges the config spec enforces.
    pub fn clamped(self) -> Self {
        Self {
            max_schematics: self.max_schematics.max(1),
            max_total_schematic_size_kb: self.max_total_schematic_size_kb.max(16),
            max_schematic_packet_size: self.max_schematic_packet_size.clamp(256, 32767),
            schematic_idle_timeout: self.schematic_idle_timeout.max(100),
        }
    }
    pub fn max_bytes(&self) -> u64 {
        self.max_total_schematic_size_kb * 1000
    }
}

/// Client side delay between upload packets (`PACKET_DELAY`).
pub const CLIENT_PACKET_DELAY: i32 = 10;
/// `canPlayerUse`: squared distance to the table centre.
pub const TABLE_USE_DISTANCE_SQ: f64 = 64.0;
pub const TABLE_INPUT_SLOT: usize = 0;
pub const TABLE_OUTPUT_SLOT: usize = 1;

/// `SchematicTableMenu` slot filter (`mayPlace`): the input takes the empty
/// schematic, the Schematic and Quill and the schematic; the output takes nothing.
pub fn table_may_place(slot: usize, item: &str) -> bool {
    match slot {
        TABLE_INPUT_SLOT => item == EMPTY_SCHEMATIC_ITEM || item == SCHEMATIC_AND_QUILL_ITEM || item == SCHEMATIC_ITEM,
        _ => false,
    }
}

/// Block entity state of the Schematic Table.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SchematicTable {
    pub input_present: bool,
    pub output_present: bool,
    pub is_uploading: bool,
    pub uploading_schematic: Option<String>,
    pub uploading_progress: f32,
    pub send_update: bool,
}

impl SchematicTable {
    /// `SchematicTableMenu.canWrite`.
    pub fn can_write(&self) -> bool {
        self.input_present && !self.output_present
    }

    /// `startUpload`: the whole input stack is removed.
    pub fn start_upload(&mut self, schematic: &str) {
        self.is_uploading = true;
        self.uploading_progress = 0.0;
        self.uploading_schematic = Some(schematic.to_owned());
        self.send_update = true;
        self.input_present = false;
    }

    /// `finishUpload`: the output slot is left alone (the loader fills it).
    pub fn finish_upload(&mut self) {
        self.is_uploading = false;
        self.uploading_progress = 0.0;
        self.uploading_schematic = None;
        self.send_update = true;
    }

    pub fn set_progress(&mut self, progress: f32) {
        self.uploading_progress = progress;
        self.send_update = true;
    }

    /// `tick`: returns true when a client block update (flags 6) must be sent.
    pub fn tick(&mut self) -> bool {
        std::mem::take(&mut self.send_update)
    }

    pub fn can_player_use(distance_sq_to_center: f64) -> bool {
        distance_sq_to_center <= TABLE_USE_DISTANCE_SQ
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UploadReject {
    /// Player or schematic name escapes its folder.
    PathEscapes,
    NotNbt,
    /// Reported to the player with the actual and the allowed size in KB.
    TooLarge { size_kb: u64, max_kb: u64 },
    /// An upload with this `player/schematic` id is already running.
    AlreadyUploading,
    NoTable,
}

/// Lexically normalised components of `rel`. `None` if it is absolute or climbs
/// above its root (`..` past the start).
fn normalize(rel: &str) -> Option<Vec<String>> {
    if rel.starts_with('/') || rel.starts_with('\\') || rel.contains(':') {
        return None;
    }
    let mut out: Vec<String> = Vec::new();
    for part in rel.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                out.pop()?;
            }
            other => out.push(other.to_owned()),
        }
    }
    Some(out)
}

/// `playerDir` under the base directory and `target` under `playerDir`.
pub fn path_is_safe(player: &str, schematic: &str) -> bool {
    if schematic.starts_with('/') || schematic.starts_with('\\') || schematic.contains(':') {
        return false; // an absolute path would replace the player folder
    }
    match normalize(player) {
        Some(p) if !p.is_empty() => {
            normalize(&format!("{}/{}", p.join("/"), schematic)).is_some_and(|full| full.len() >= p.len() && full[..p.len()] == p[..])
        }
        _ => false,
    }
}

/// One entry of the player's schematic folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderEntry {
    pub name: String,
    pub modified: i64,
    pub is_dir: bool,
}

/// Outcome of `handleNewUpload` validation: files to delete before opening the stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewUploadPlan {
    pub delete: Vec<String>,
    pub id: String,
}

/// Oldest regular file (smallest `modified`; first one on ties).
fn oldest_file(entries: &[FolderEntry]) -> Option<&FolderEntry> {
    entries.iter().filter(|e| !e.is_dir).min_by_key(|e| e.modified)
}

/// `handleNewUpload` checks in source order: path security, `.nbt`, size, duplicate
/// session, then (after the table lookup) deletion of a same-name file and, if the
/// folder then holds `max_schematics` or more entries (directories included), the
/// oldest regular file.
pub fn plan_new_upload(
    player: &str,
    schematic: &str,
    size: u64,
    active_ids: &dyn Fn(&str) -> bool,
    table_exists: bool,
    folder: &[FolderEntry],
    cfg: &UploadConfig,
) -> Result<NewUploadPlan, UploadReject> {
    if !path_is_safe(player, schematic) {
        return Err(UploadReject::PathEscapes);
    }
    if !schematic.ends_with(".nbt") {
        return Err(UploadReject::NotNbt);
    }
    if size > cfg.max_bytes() {
        return Err(UploadReject::TooLarge { size_kb: size / 1000, max_kb: cfg.max_total_schematic_size_kb });
    }
    let id = format!("{player}/{schematic}");
    if active_ids(&id) {
        return Err(UploadReject::AlreadyUploading);
    }
    if !table_exists {
        return Err(UploadReject::NoTable);
    }
    let mut delete = Vec::new();
    let mut remaining: Vec<FolderEntry> = Vec::new();
    for e in folder {
        if e.name == schematic {
            delete.push(e.name.clone());
        } else {
            remaining.push(e.clone());
        }
    }
    if remaining.len() >= cfg.max_schematics {
        if let Some(old) = oldest_file(&remaining) {
            delete.push(old.name.clone());
        }
    }
    Ok(NewUploadPlan { delete, id })
}

/// `tryDeleteOldest` used by the instant (Schematic and Quill) upload: nothing
/// is deleted below the cap; at the cap the oldest entry goes (the original aborts
/// on an IO error, which has no pure equivalent).
pub fn instant_upload_delete(folder: &[FolderEntry], cfg: &UploadConfig) -> Option<String> {
    if folder.len() < cfg.max_schematics {
        return None;
    }
    folder.iter().min_by_key(|e| e.modified).map(|e| e.name.clone())
}

#[derive(Clone, Debug, PartialEq)]
pub enum WriteOutcome {
    /// Chunk accepted; table progress is `bytes_uploaded / total`.
    Accepted { progress: f32 },
    /// Upload must be cancelled (oversize packet or more bytes than announced).
    Cancel,
    UnknownSession,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadSession {
    pub total: u64,
    pub uploaded: u64,
    pub idle: i32,
    pub table_pos: [i32; 3],
}

/// Active uploads keyed by `player/schematic`.
#[derive(Clone, Debug, Default)]
pub struct UploadManager {
    pub sessions: BTreeMap<String, UploadSession>,
}

impl UploadManager {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn is_active(&self, id: &str) -> bool {
        self.sessions.contains_key(id)
    }
    pub fn begin(&mut self, id: &str, total: u64, table_pos: [i32; 3]) {
        self.sessions.insert(id.to_owned(), UploadSession { total, uploaded: 0, idle: 0, table_pos });
    }

    /// `handleWriteRequest`: the byte count is added before validation.
    pub fn write(&mut self, id: &str, len: usize, cfg: &UploadConfig) -> WriteOutcome {
        let Some(s) = self.sessions.get_mut(id) else {
            return WriteOutcome::UnknownSession;
        };
        s.uploaded += len as u64;
        if len > cfg.max_schematic_packet_size || s.uploaded > s.total {
            self.sessions.remove(id);
            return WriteOutcome::Cancel;
        }
        s.idle = 0;
        let progress = if s.total == 0 { 0.0 } else { s.uploaded as f32 / s.total as f32 };
        WriteOutcome::Accepted { progress }
    }

    /// Server tick idle timeout: `idleTime++ > timeout`. Returns ids dropped
    /// (the adapter closes the stream, deletes the partial file, releases the table).
    pub fn tick(&mut self, cfg: &UploadConfig) -> Vec<String> {
        let mut dropped = Vec::new();
        for (id, s) in self.sessions.iter_mut() {
            let old = s.idle;
            s.idle += 1;
            if old > cfg.schematic_idle_timeout {
                dropped.push(id.clone());
            }
        }
        for id in &dropped {
            self.sessions.remove(id);
        }
        dropped
    }

    /// `handleFinishedUpload`: removes the session; `None` if unknown.
    pub fn finish(&mut self, id: &str) -> Option<UploadSession> {
        self.sessions.remove(id)
    }

    pub fn cancel(&mut self, id: &str) -> Option<UploadSession> {
        self.sessions.remove(id)
    }
}

/// `ClientSchematicLoader.validateSizeLimitation`: skipped in singleplayer,
/// otherwise `size > kb * 1000` fails (a file exactly at the limit passes).
pub fn client_validate_size(size: u64, singleplayer: bool, cfg: &UploadConfig) -> bool {
    singleplayer || size <= cfg.max_bytes()
}

/// GZIP magic bytes checked before starting an upload.
pub fn is_gzip(header: &[u8]) -> bool {
    header.len() >= 2 && header[0] == 0x1F && header[1] == 0x8B
}

/// Payload sizes of the write packets for a file (one per `CLIENT_PACKET_DELAY`
/// ticks); the original adds a final finish packet after the last short read.
pub fn packet_sizes(file_len: u64, cfg: &UploadConfig) -> Vec<usize> {
    let max = cfg.max_schematic_packet_size as u64;
    let mut out = Vec::new();
    let mut left = file_len;
    while left > 0 {
        let n = left.min(max);
        out.push(n as usize);
        left -= n;
    }
    out
}

/// Natural ordering used for the client's schematic list: `.nbt` stripped,
/// digit runs compared numerically, other characters by code point, ties by length.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let strip = |s: &str| s.strip_suffix(".nbt").unwrap_or(s).to_owned();
    let (a, b) = (strip(a), strip(b));
    let (ac, bc): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let (mut i, mut j) = (0, 0);
    while i < ac.len() && j < bc.len() {
        if ac[i].is_ascii_digit() && bc[j].is_ascii_digit() {
            let si = i;
            while i < ac.len() && ac[i].is_ascii_digit() {
                i += 1;
            }
            let sj = j;
            while j < bc.len() && bc[j].is_ascii_digit() {
                j += 1;
            }
            let na: String = ac[si..i].iter().collect::<String>().trim_start_matches('0').to_owned();
            let nb: String = bc[sj..j].iter().collect::<String>().trim_start_matches('0').to_owned();
            let ord = na.len().cmp(&nb.len()).then_with(|| na.cmp(&nb));
            if ord != Ordering::Equal {
                return ord;
            }
        } else {
            if ac[i] != bc[j] {
                return ac[i].cmp(&bc[j]);
            }
            i += 1;
            j += 1;
        }
    }
    ac.len().cmp(&bc.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> UploadConfig {
        UploadConfig::default()
    }
    fn entry(name: &str, modified: i64, is_dir: bool) -> FolderEntry {
        FolderEntry { name: name.into(), modified, is_dir }
    }

    #[test]
    fn config_defaults_and_clamps() {
        let c = cfg();
        assert_eq!((c.max_schematics, c.max_total_schematic_size_kb), (10, 256));
        assert_eq!((c.max_schematic_packet_size, c.schematic_idle_timeout), (1024, 600));
        assert_eq!(c.max_bytes(), 256_000);
        let k = UploadConfig {
            max_schematics: 0,
            max_total_schematic_size_kb: 1,
            max_schematic_packet_size: 99_999,
            schematic_idle_timeout: 5,
        }
        .clamped();
        assert_eq!((k.max_schematics, k.max_total_schematic_size_kb), (1, 16));
        assert_eq!((k.max_schematic_packet_size, k.schematic_idle_timeout), (32767, 100));
    }

    #[test]
    fn slot_filters() {
        assert!(table_may_place(0, SCHEMATIC_ITEM));
        assert!(table_may_place(0, EMPTY_SCHEMATIC_ITEM));
        assert!(table_may_place(0, SCHEMATIC_AND_QUILL_ITEM));
        assert!(!table_may_place(0, "minecraft:stone"));
        assert!(!table_may_place(1, SCHEMATIC_ITEM));
    }

    #[test]
    fn table_state_transitions() {
        let mut t = SchematicTable { input_present: true, ..Default::default() };
        assert!(t.can_write());
        t.output_present = true;
        assert!(!t.can_write());
        t.output_present = false;
        t.start_upload("a.nbt");
        assert!(t.is_uploading && !t.input_present && t.uploading_schematic.as_deref() == Some("a.nbt"));
        t.set_progress(0.5);
        assert!(t.tick());
        assert!(!t.tick());
        t.finish_upload();
        assert!(!t.is_uploading && t.uploading_progress == 0.0 && t.uploading_schematic.is_none());
        assert!(SchematicTable::can_player_use(64.0));
        assert!(!SchematicTable::can_player_use(64.1));
    }

    #[test]
    fn path_security() {
        assert!(path_is_safe("Steve", "house.nbt"));
        assert!(path_is_safe("Steve", "sub/house.nbt"));
        assert!(path_is_safe("Steve", "a/../house.nbt"));
        assert!(!path_is_safe("Steve", "../Alex/house.nbt"));
        assert!(!path_is_safe("Steve", "/etc/x.nbt"));
        assert!(!path_is_safe("Steve", "C:\\x.nbt"));
        assert!(!path_is_safe("..", "x.nbt"));
        assert!(!path_is_safe("", "x.nbt"));
        assert!(!path_is_safe("a/../..", "x.nbt"));
    }

    #[test]
    fn new_upload_rejections_in_source_order() {
        let none = |_: &str| false;
        assert_eq!(
            plan_new_upload("S", "../x.nbt", 1, &none, true, &[], &cfg()),
            Err(UploadReject::PathEscapes)
        );
        assert_eq!(plan_new_upload("S", "x.txt", 1, &none, true, &[], &cfg()), Err(UploadReject::NotNbt));
        assert_eq!(
            plan_new_upload("S", "x.nbt", 256_001, &none, true, &[], &cfg()),
            Err(UploadReject::TooLarge { size_kb: 256, max_kb: 256 })
        );
        assert!(plan_new_upload("S", "x.nbt", 256_000, &none, true, &[], &cfg()).is_ok());
        let busy = |id: &str| id == "S/x.nbt";
        assert_eq!(plan_new_upload("S", "x.nbt", 1, &busy, true, &[], &cfg()), Err(UploadReject::AlreadyUploading));
        assert_eq!(plan_new_upload("S", "x.nbt", 1, &none, false, &[], &cfg()), Err(UploadReject::NoTable));
    }

    #[test]
    fn new_upload_replaces_same_name_and_oldest_at_cap() {
        let none = |_: &str| false;
        let small = UploadConfig { max_schematics: 2, ..cfg() };
        let folder = vec![entry("a.nbt", 5, false), entry("b.nbt", 1, false), entry("x.nbt", 9, false)];
        let plan = plan_new_upload("S", "x.nbt", 10, &none, true, &folder, &small).unwrap();
        // same name is removed first, then 2 files remain == cap -> oldest (b) goes too
        assert_eq!(plan.delete, vec!["x.nbt".to_string(), "b.nbt".to_string()]);
        assert_eq!(plan.id, "S/x.nbt");
        let under = plan_new_upload("S", "x.nbt", 10, &none, true, &folder[..2], &small).unwrap();
        assert_eq!(under.delete, vec!["b.nbt".to_string()]);
        let free = plan_new_upload("S", "x.nbt", 10, &none, true, &folder[..1], &small).unwrap();
        assert!(free.delete.is_empty());
    }

    #[test]
    fn directories_count_towards_cap_but_are_never_deleted() {
        let none = |_: &str| false;
        let small = UploadConfig { max_schematics: 1, ..cfg() };
        let folder = vec![entry("dir", 0, true)];
        let plan = plan_new_upload("S", "x.nbt", 1, &none, true, &folder, &small).unwrap();
        assert!(plan.delete.is_empty());
    }

    #[test]
    fn instant_upload_deletes_oldest_only_at_cap() {
        let small = UploadConfig { max_schematics: 2, ..cfg() };
        let one = vec![entry("a", 3, false)];
        assert_eq!(instant_upload_delete(&one, &small), None);
        let two = vec![entry("a", 3, false), entry("b", 1, false)];
        assert_eq!(instant_upload_delete(&two, &small), Some("b".to_string()));
        let one_cap = UploadConfig { max_schematics: 1, ..cfg() };
        assert_eq!(instant_upload_delete(&[], &one_cap), None);
    }

    #[test]
    fn write_accounting_and_cancel_rules() {
        let mut m = UploadManager::new();
        m.begin("S/x.nbt", 2000, [0; 3]);
        assert_eq!(m.write("nope", 1, &cfg()), WriteOutcome::UnknownSession);
        assert_eq!(m.write("S/x.nbt", 1000, &cfg()), WriteOutcome::Accepted { progress: 0.5 });
        assert_eq!(m.write("S/x.nbt", 1000, &cfg()), WriteOutcome::Accepted { progress: 1.0 });
        assert_eq!(m.write("S/x.nbt", 1, &cfg()), WriteOutcome::Cancel);
        assert!(!m.is_active("S/x.nbt"));
        m.begin("S/y.nbt", 5000, [0; 3]);
        assert_eq!(m.write("S/y.nbt", 1025, &cfg()), WriteOutcome::Cancel);
    }

    #[test]
    fn idle_timeout_uses_post_increment() {
        let mut m = UploadManager::new();
        m.begin("a", 10, [0; 3]);
        let c = cfg();
        // dropped when the pre-increment idle count exceeds 600, i.e. on tick 602
        for _ in 0..601 {
            assert!(m.tick(&c).is_empty());
        }
        assert_eq!(m.tick(&c), vec!["a".to_string()]);
        m.begin("b", 10, [0; 3]);
        for _ in 0..300 {
            m.tick(&c);
        }
        m.write("b", 1, &c);
        assert_eq!(m.sessions["b"].idle, 0);
    }

    #[test]
    fn finish_returns_session() {
        let mut m = UploadManager::new();
        m.begin("a", 10, [1, 2, 3]);
        assert_eq!(m.finish("a").unwrap().table_pos, [1, 2, 3]);
        assert!(m.finish("a").is_none());
    }

    #[test]
    fn client_size_and_format_checks() {
        assert!(client_validate_size(256_000, false, &cfg()));
        assert!(!client_validate_size(256_001, false, &cfg()));
        assert!(client_validate_size(10_000_000, true, &cfg()));
        assert!(is_gzip(&[0x1F, 0x8B, 8]));
        assert!(!is_gzip(&[0x1F]));
        assert!(!is_gzip(b"NBT"));
    }

    #[test]
    fn packet_chunks() {
        assert_eq!(packet_sizes(0, &cfg()), Vec::<usize>::new());
        assert_eq!(packet_sizes(2500, &cfg()), vec![1024, 1024, 452]);
        assert_eq!(packet_sizes(2048, &cfg()), vec![1024, 1024]);
    }

    #[test]
    fn natural_sort() {
        let mut v = vec!["house10.nbt", "house2.nbt", "House.nbt", "house1.nbt", "a.nbt"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, ["House.nbt", "a.nbt", "house1.nbt", "house2.nbt", "house10.nbt"]);
        // equal numeric value: the longer spelling sorts after (length tie-break)
        assert_eq!(natural_cmp("a01.nbt", "a1.nbt"), Ordering::Greater);
        assert_eq!(natural_cmp("a.nbt", "ab.nbt"), Ordering::Less);
    }
}
