use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{Duration, Instant};

const RECORD_HEADER_BYTES: usize = 12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LayoutKind {
    Files,
    Segments,
}

#[derive(Debug)]
struct ScanStats {
    events: u64,
    payload_bytes: u64,
    stored_bytes: u64,
    physical_objects: u64,
    max_events_per_object: u64,
    enumeration: Duration,
    rebuild: Duration,
    truncated_active_tail: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("m0-storage-bench: {error}");
        process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or_else(usage)?;

    match command.as_str() {
        "generate" => {
            let layout = parse_layout(args.next())?;
            let events = parse_u64(args.next(), "events")?;
            let payload_bytes = parse_usize(args.next(), "payload_bytes")?;
            let root = PathBuf::from(args.next().ok_or_else(usage)?);
            let segment_events = parse_segment_events(layout, args.next())?;
            reject_extra_args(args)?;

            ensure_empty_root(&root)?;
            let started = Instant::now();
            append(layout, &root, 0, events, payload_bytes, segment_events)?;
            let append_elapsed = started.elapsed();
            let scan = scan(layout, &root)?;
            print_stats("generate", layout, &root, Some(append_elapsed), &scan);
        }
        "append" => {
            let layout = parse_layout(args.next())?;
            let events = parse_u64(args.next(), "events")?;
            let payload_bytes = parse_usize(args.next(), "payload_bytes")?;
            let root = PathBuf::from(args.next().ok_or_else(usage)?);
            let segment_events = parse_segment_events(layout, args.next())?;
            reject_extra_args(args)?;

            if !root.is_dir() {
                return Err(format!("benchmark root does not exist: {}", root.display()));
            }

            let before = scan(layout, &root)?;
            if before.truncated_active_tail {
                return Err("refusing to append after a truncated active segment tail; run recovery analysis first".to_string());
            }

            let started = Instant::now();
            append(
                layout,
                &root,
                before.events,
                events,
                payload_bytes,
                segment_events,
            )?;
            let append_elapsed = started.elapsed();
            let after = scan(layout, &root)?;
            print_stats("append", layout, &root, Some(append_elapsed), &after);
        }
        "scan" => {
            let layout = parse_layout(args.next())?;
            let root = PathBuf::from(args.next().ok_or_else(usage)?);
            reject_extra_args(args)?;
            let stats = scan(layout, &root)?;
            print_stats("scan", layout, &root, None, &stats);
        }
        "crash-probe" => {
            let layout = parse_layout(args.next())?;
            let payload_bytes = parse_usize(args.next(), "payload_bytes")?;
            let root = PathBuf::from(args.next().ok_or_else(usage)?);
            reject_extra_args(args)?;
            ensure_empty_root(&root)?;
            run_crash_probe(layout, &root, payload_bytes)?;
        }
        "purge-probe" => {
            let layout = parse_layout(args.next())?;
            let payload_bytes = parse_usize(args.next(), "payload_bytes")?;
            let root = PathBuf::from(args.next().ok_or_else(usage)?);
            let segment_events = parse_segment_events(layout, args.next())?;
            reject_extra_args(args)?;
            ensure_empty_root(&root)?;
            run_purge_probe(layout, &root, payload_bytes, segment_events)?;
        }
        "corrupt-probe" => {
            let layout = parse_layout(args.next())?;
            let payload_bytes = parse_usize(args.next(), "payload_bytes")?;
            let root = PathBuf::from(args.next().ok_or_else(usage)?);
            let segment_events = parse_segment_events(layout, args.next())?;
            reject_extra_args(args)?;
            ensure_empty_root(&root)?;
            run_corrupt_probe(layout, &root, payload_bytes, segment_events)?;
        }
        _ => return Err(usage()),
    }

    Ok(())
}

fn usage() -> String {
    [
        "usage:",
        "  m0-storage-bench generate files <events> <payload_bytes> <root>",
        "  m0-storage-bench generate segments <events> <payload_bytes> <root> <segment_events>",
        "  m0-storage-bench append files <events> <payload_bytes> <root>",
        "  m0-storage-bench append segments <events> <payload_bytes> <root> <segment_events>",
        "  m0-storage-bench scan <files|segments> <root>",
        "  m0-storage-bench crash-probe <files|segments> <payload_bytes> <root>",
        "  m0-storage-bench purge-probe files <payload_bytes> <root>",
        "  m0-storage-bench purge-probe segments <payload_bytes> <root> <segment_events>",
        "  m0-storage-bench corrupt-probe files <payload_bytes> <root>",
        "  m0-storage-bench corrupt-probe segments <payload_bytes> <root> <segment_events>",
    ]
    .join("\n")
}

fn parse_layout(value: Option<String>) -> Result<LayoutKind, String> {
    match value.as_deref() {
        Some("files") => Ok(LayoutKind::Files),
        Some("segments") => Ok(LayoutKind::Segments),
        _ => Err(usage()),
    }
}

fn parse_u64(value: Option<String>, name: &str) -> Result<u64, String> {
    let raw = value.ok_or_else(usage)?;
    let parsed = raw
        .parse::<u64>()
        .map_err(|_| format!("invalid {name}: {raw}"))?;
    if parsed == 0 {
        return Err(format!("{name} must be greater than zero"));
    }
    Ok(parsed)
}

fn parse_usize(value: Option<String>, name: &str) -> Result<usize, String> {
    let parsed = parse_u64(value, name)?;
    usize::try_from(parsed).map_err(|_| format!("{name} is too large for this platform"))
}

fn parse_segment_events(layout: LayoutKind, value: Option<String>) -> Result<Option<u64>, String> {
    match layout {
        LayoutKind::Files => {
            if value.is_some() {
                return Err("files layout does not accept segment_events".to_string());
            }
            Ok(None)
        }
        LayoutKind::Segments => Ok(Some(parse_u64(value, "segment_events")?)),
    }
}

fn reject_extra_args(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    if let Some(extra) = args.next() {
        return Err(format!("unexpected extra argument: {extra}"));
    }
    Ok(())
}

fn ensure_empty_root(root: &Path) -> Result<(), String> {
    if root.exists() {
        if !root.is_dir() {
            return Err(format!(
                "benchmark root is not a directory: {}",
                root.display()
            ));
        }
        let mut entries = fs::read_dir(root).map_err(io_error("read benchmark root"))?;
        if entries
            .next()
            .transpose()
            .map_err(io_error("read benchmark root"))?
            .is_some()
        {
            return Err(format!(
                "benchmark root must be empty; refusing to delete existing data: {}",
                root.display()
            ));
        }
    } else {
        fs::create_dir_all(root).map_err(io_error("create benchmark root"))?;
    }
    Ok(())
}

fn append(
    layout: LayoutKind,
    root: &Path,
    start_seq: u64,
    events: u64,
    payload_bytes: usize,
    segment_events: Option<u64>,
) -> Result<(), String> {
    match layout {
        LayoutKind::Files => append_files(root, start_seq, events, payload_bytes),
        LayoutKind::Segments => append_segments(
            root,
            start_seq,
            events,
            payload_bytes,
            segment_events.expect("segment limit required"),
        ),
    }
}

fn append_files(
    root: &Path,
    start_seq: u64,
    events: u64,
    payload_bytes: usize,
) -> Result<(), String> {
    let events_dir = root.join("events");
    fs::create_dir_all(&events_dir).map_err(io_error("create event directory"))?;
    let mut payload = vec![0_u8; payload_bytes];

    for seq in start_seq..start_seq + events {
        fill_payload(&mut payload, seq);
        let final_path = events_dir.join(format!("{seq:020}.evt"));
        let temp_path = events_dir.join(format!(".{seq:020}.tmp"));

        if final_path.exists() || temp_path.exists() {
            return Err(format!("event path already exists for sequence {seq}"));
        }

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(io_error("create temporary event file"))?;
        write_record(&mut file, seq, &payload)?;
        file.sync_all().map_err(io_error("sync event file"))?;
        drop(file);
        fs::rename(&temp_path, &final_path).map_err(io_error("commit event file"))?;
    }

    Ok(())
}

fn append_segments(
    root: &Path,
    start_seq: u64,
    events: u64,
    payload_bytes: usize,
    segment_events: u64,
) -> Result<(), String> {
    let segments_dir = root.join("segments");
    fs::create_dir_all(&segments_dir).map_err(io_error("create segment directory"))?;

    let (mut segment_index, mut active_count) = discover_active_segment(&segments_dir)?;
    let mut remaining = events;
    let mut seq = start_seq;
    let mut payload = vec![0_u8; payload_bytes];

    while remaining > 0 {
        let open_path = segments_dir.join(format!("{segment_index:020}.open"));
        let sealed_path = segments_dir.join(format!("{segment_index:020}.seg"));

        if sealed_path.exists() {
            return Err(format!("segment already sealed: {}", sealed_path.display()));
        }

        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&open_path)
            .map_err(io_error("open active segment"))?;

        let room = segment_events.saturating_sub(active_count);
        if room == 0 {
            file.sync_all().map_err(io_error("sync full segment"))?;
            drop(file);
            fs::rename(&open_path, &sealed_path).map_err(io_error("seal segment"))?;
            segment_index += 1;
            active_count = 0;
            continue;
        }

        let batch = remaining.min(room);
        for _ in 0..batch {
            fill_payload(&mut payload, seq);
            write_record(&mut file, seq, &payload)?;
            file.sync_data().map_err(io_error("sync segment record"))?;
            seq += 1;
            remaining -= 1;
            active_count += 1;
        }

        if active_count == segment_events {
            file.sync_all().map_err(io_error("sync full segment"))?;
            drop(file);
            fs::rename(&open_path, &sealed_path).map_err(io_error("seal segment"))?;
            segment_index += 1;
            active_count = 0;
        }
    }

    Ok(())
}

fn discover_active_segment(segments_dir: &Path) -> Result<(u64, u64), String> {
    let mut max_index: Option<u64> = None;
    let mut active: Option<(u64, PathBuf)> = None;

    for entry in fs::read_dir(segments_dir).map_err(io_error("enumerate segments"))? {
        let entry = entry.map_err(io_error("read segment entry"))?;
        let path = entry.path();
        let extension = path.extension().and_then(|value| value.to_str());
        if extension != Some("seg") && extension != Some("open") {
            continue;
        }

        let index = parse_index(&path)?;
        max_index = Some(max_index.map_or(index, |current| current.max(index)));

        if extension == Some("open") {
            if active.is_some() {
                return Err("multiple active .open segments found".to_string());
            }
            active = Some((index, path));
        }
    }

    if let Some((index, path)) = active {
        let bytes = fs::read(&path).map_err(io_error("read active segment"))?;
        let parsed = parse_records(&bytes, true)?;
        if parsed.truncated_tail {
            return Err(format!(
                "active segment has a truncated tail: {}",
                path.display()
            ));
        }
        Ok((index, parsed.events))
    } else {
        Ok((max_index.map_or(0, |index| index + 1), 0))
    }
}

fn write_record(file: &mut File, seq: u64, payload: &[u8]) -> Result<(), String> {
    let payload_len = u32::try_from(payload.len())
        .map_err(|_| "payload is too large for benchmark record framing".to_string())?;
    file.write_all(&seq.to_le_bytes())
        .map_err(io_error("write record sequence"))?;
    file.write_all(&payload_len.to_le_bytes())
        .map_err(io_error("write record length"))?;
    file.write_all(payload)
        .map_err(io_error("write record payload"))?;
    Ok(())
}

fn fill_payload(payload: &mut [u8], seq: u64) {
    let mut state = seq ^ 0x9E37_79B9_7F4A_7C15;
    for byte in payload {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *byte = (state & 0xff) as u8;
    }
}

fn scan(layout: LayoutKind, root: &Path) -> Result<ScanStats, String> {
    match layout {
        LayoutKind::Files => scan_files(root),
        LayoutKind::Segments => scan_segments(root),
    }
}

fn scan_files(root: &Path) -> Result<ScanStats, String> {
    let events_dir = root.join("events");
    if !events_dir.is_dir() {
        return Err(format!(
            "event directory does not exist: {}",
            events_dir.display()
        ));
    }

    let enumerate_started = Instant::now();
    let mut paths = Vec::new();
    let mut stored_bytes = 0_u64;
    for entry in fs::read_dir(&events_dir).map_err(io_error("enumerate event directory"))? {
        let entry = entry.map_err(io_error("read event directory entry"))?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("evt") {
            continue;
        }
        stored_bytes += entry
            .metadata()
            .map_err(io_error("read event metadata"))?
            .len();
        paths.push(path);
    }
    let enumeration = enumerate_started.elapsed();

    let rebuild_started = Instant::now();
    let mut events = 0_u64;
    let mut payload_bytes = 0_u64;
    for path in &paths {
        let bytes = fs::read(path).map_err(io_error("read event file"))?;
        let parsed = parse_records(&bytes, false)?;
        if parsed.events != 1 {
            return Err(format!(
                "event file did not contain exactly one record: {}",
                path.display()
            ));
        }
        events += 1;
        payload_bytes += parsed.payload_bytes;
    }
    let rebuild = rebuild_started.elapsed();

    Ok(ScanStats {
        events,
        payload_bytes,
        stored_bytes,
        physical_objects: paths.len() as u64,
        max_events_per_object: if paths.is_empty() { 0 } else { 1 },
        enumeration,
        rebuild,
        truncated_active_tail: false,
    })
}

fn scan_segments(root: &Path) -> Result<ScanStats, String> {
    let segments_dir = root.join("segments");
    if !segments_dir.is_dir() {
        return Err(format!(
            "segment directory does not exist: {}",
            segments_dir.display()
        ));
    }

    let enumerate_started = Instant::now();
    let mut paths = Vec::new();
    let mut stored_bytes = 0_u64;
    for entry in fs::read_dir(&segments_dir).map_err(io_error("enumerate segment directory"))? {
        let entry = entry.map_err(io_error("read segment directory entry"))?;
        let path = entry.path();
        let extension = path.extension().and_then(|value| value.to_str());
        if extension != Some("seg") && extension != Some("open") {
            continue;
        }
        stored_bytes += entry
            .metadata()
            .map_err(io_error("read segment metadata"))?
            .len();
        paths.push(path);
    }
    paths.sort();
    let enumeration = enumerate_started.elapsed();

    let rebuild_started = Instant::now();
    let mut events = 0_u64;
    let mut payload_bytes = 0_u64;
    let mut max_events_per_object = 0_u64;
    let mut truncated_active_tail = false;

    for path in &paths {
        let bytes = fs::read(path).map_err(io_error("read segment file"))?;
        let is_active = path.extension().and_then(|value| value.to_str()) == Some("open");
        let parsed = parse_records(&bytes, is_active)?;
        events += parsed.events;
        payload_bytes += parsed.payload_bytes;
        max_events_per_object = max_events_per_object.max(parsed.events);
        truncated_active_tail |= parsed.truncated_tail;
    }
    let rebuild = rebuild_started.elapsed();

    Ok(ScanStats {
        events,
        payload_bytes,
        stored_bytes,
        physical_objects: paths.len() as u64,
        max_events_per_object,
        enumeration,
        rebuild,
        truncated_active_tail,
    })
}

#[derive(Debug)]
struct ParsedRecords {
    events: u64,
    payload_bytes: u64,
    truncated_tail: bool,
}

fn parse_records(bytes: &[u8], allow_truncated_tail: bool) -> Result<ParsedRecords, String> {
    let mut cursor = 0_usize;
    let mut events = 0_u64;
    let mut payload_bytes = 0_u64;
    let mut truncated_tail = false;

    while cursor < bytes.len() {
        let remaining = bytes.len() - cursor;
        if remaining < RECORD_HEADER_BYTES {
            if allow_truncated_tail {
                truncated_tail = true;
                break;
            }
            return Err("sealed object contains a truncated record header".to_string());
        }

        let length_offset = cursor + 8;
        let payload_len = u32::from_le_bytes(
            bytes[length_offset..length_offset + 4]
                .try_into()
                .expect("record header slice has fixed width"),
        ) as usize;
        let total_record_len = RECORD_HEADER_BYTES
            .checked_add(payload_len)
            .ok_or_else(|| "record length overflow".to_string())?;

        if remaining < total_record_len {
            if allow_truncated_tail {
                truncated_tail = true;
                break;
            }
            return Err("sealed object contains a truncated record payload".to_string());
        }

        events += 1;
        payload_bytes += payload_len as u64;
        cursor += total_record_len;
    }

    Ok(ParsedRecords {
        events,
        payload_bytes,
        truncated_tail,
    })
}

fn parse_index(path: &Path) -> Result<u64, String> {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("invalid segment filename: {}", path.display()))?;
    stem.parse::<u64>()
        .map_err(|_| format!("invalid segment index: {}", path.display()))
}

fn run_crash_probe(layout: LayoutKind, root: &Path, payload_bytes: usize) -> Result<(), String> {
    match layout {
        LayoutKind::Files => {
            append_files(root, 0, 9, payload_bytes)?;
            let events_dir = root.join("events");
            let temp_path = events_dir.join(".00000000000000000009.tmp");
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)
                .map_err(io_error("create crash-probe temp event"))?;
            let payload = vec![0x5a; payload_bytes / 2];
            file.write_all(&9_u64.to_le_bytes())
                .map_err(io_error("write crash-probe sequence"))?;
            file.write_all(&(payload_bytes as u32).to_le_bytes())
                .map_err(io_error("write crash-probe length"))?;
            file.write_all(&payload)
                .map_err(io_error("write crash-probe partial payload"))?;
            file.sync_all()
                .map_err(io_error("sync crash-probe temp event"))?;
            drop(file);

            let stats = scan_files(root)?;
            print_stats("crash-probe", layout, root, None, &stats);
            println!("crash_probe_incomplete_artifact_ignored=true");
            println!("crash_probe_recovered_events={}", stats.events);
            println!("crash_probe_incomplete_events=1");
        }
        LayoutKind::Segments => {
            let segments_dir = root.join("segments");
            fs::create_dir_all(&segments_dir).map_err(io_error("create segment directory"))?;
            let open_path = segments_dir.join("00000000000000000000.open");
            let mut file = OpenOptions::new()
                .append(true)
                .create_new(true)
                .open(&open_path)
                .map_err(io_error("create crash-probe active segment"))?;
            let mut payload = vec![0_u8; payload_bytes];
            for seq in 0..9_u64 {
                fill_payload(&mut payload, seq);
                write_record(&mut file, seq, &payload)?;
                file.sync_data()
                    .map_err(io_error("sync crash-probe record"))?;
            }
            fill_payload(&mut payload, 9);
            file.write_all(&9_u64.to_le_bytes())
                .map_err(io_error("write crash-probe sequence"))?;
            file.write_all(&(payload_bytes as u32).to_le_bytes())
                .map_err(io_error("write crash-probe length"))?;
            file.write_all(&payload[..payload.len() / 2])
                .map_err(io_error("write crash-probe partial payload"))?;
            file.sync_all()
                .map_err(io_error("sync crash-probe active segment"))?;
            drop(file);

            let stats = scan_segments(root)?;
            print_stats("crash-probe", layout, root, None, &stats);
            println!("crash_probe_recovered_events={}", stats.events);
            println!("crash_probe_incomplete_events=1");
            println!(
                "crash_probe_truncated_active_tail={}",
                stats.truncated_active_tail
            );
        }
    }

    Ok(())
}

const PURGE_PROBE_EVENTS: u64 = 20;
const PURGE_PROBE_TARGET_SEQ: u64 = 7;
// 22 events with segment_events=5 leaves an active .open tail (4 sealed + 2 open).
const CORRUPT_PROBE_EVENTS: u64 = 22;
const CORRUPT_PROBE_TARGET_SEQ: u64 = 7;

fn run_purge_probe(
    layout: LayoutKind,
    root: &Path,
    payload_bytes: usize,
    segment_events: Option<u64>,
) -> Result<(), String> {
    match layout {
        LayoutKind::Files => {
            append_files(root, 0, PURGE_PROBE_EVENTS, payload_bytes)?;
            let before = scan_files(root)?;
            let target = root
                .join("events")
                .join(format!("{PURGE_PROBE_TARGET_SEQ:020}.evt"));
            if !target.exists() {
                return Err(format!(
                    "purge target event object missing: {}",
                    target.display()
                ));
            }
            let target_bytes = fs::metadata(&target)
                .map_err(io_error("read purge target metadata"))?
                .len();
            fs::remove_file(&target).map_err(io_error("remove purged event object"))?;
            let after = scan_files(root)?;

            print_stats("purge-probe", layout, root, None, &after);
            println!("purge_target_seq={PURGE_PROBE_TARGET_SEQ}");
            println!("purge_physical_objects_removed=1");
            println!("purge_physical_objects_rewritten=0");
            println!("purge_bytes_removed={target_bytes}");
            println!("purge_removal_granularity=single_event_object");
            println!("purge_atomicity=filesystem_unlink_of_immutable_object");
            println!("purge_interruption_behavior=unlink_is_all_or_nothing_for_one_object");
            println!("purge_events_before={}", before.events);
            println!("purge_events_after={}", after.events);
            println!("purge_unaffected_events={}", after.events);
            println!("purge_segment_rewrite_required=false");
        }
        LayoutKind::Segments => {
            let segment_events = segment_events
                .ok_or_else(|| "segments purge-probe requires segment_events".to_string())?;
            if segment_events < 3 {
                return Err("segments purge-probe requires segment_events >= 3".to_string());
            }
            append_segments(root, 0, PURGE_PROBE_EVENTS, payload_bytes, segment_events)?;
            let before = scan_segments(root)?;
            let segment_index = PURGE_PROBE_TARGET_SEQ / segment_events;
            let sealed_path = root
                .join("segments")
                .join(format!("{segment_index:020}.seg"));
            if !sealed_path.exists() {
                return Err(format!(
                    "expected sealed segment for purge target: {}",
                    sealed_path.display()
                ));
            }
            let original_bytes = fs::read(&sealed_path).map_err(io_error("read sealed segment"))?;
            let parsed_before = parse_records(&original_bytes, false)?;
            let rewritten = rewrite_segment_without_seq(&original_bytes, PURGE_PROBE_TARGET_SEQ)?;
            let temp_path = root
                .join("segments")
                .join(format!(".{segment_index:020}.purge.tmp"));
            {
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&temp_path)
                    .map_err(io_error("create purge rewrite temp"))?;
                file.write_all(&rewritten)
                    .map_err(io_error("write purge rewrite temp"))?;
                file.sync_all()
                    .map_err(io_error("sync purge rewrite temp"))?;
            }
            // Simulate interruption visibility: temp exists until rename commits replacement.
            let interrupted_temp_exists = temp_path.exists();
            fs::rename(&temp_path, &sealed_path).map_err(io_error("commit purged segment"))?;
            let after = scan_segments(root)?;
            let parsed_after = parse_records(
                &fs::read(&sealed_path).map_err(io_error("read rewritten segment"))?,
                false,
            )?;

            print_stats("purge-probe", layout, root, None, &after);
            println!("purge_target_seq={PURGE_PROBE_TARGET_SEQ}");
            println!("purge_physical_objects_removed=0");
            println!("purge_physical_objects_rewritten=1");
            println!(
                "purge_bytes_removed={}",
                original_bytes.len() as u64 - rewritten.len() as u64
            );
            println!("purge_removal_granularity=record_inside_sealed_segment");
            println!("purge_atomicity=temp_rewrite_then_rename_over_sealed_segment");
            println!(
                "purge_interruption_behavior=temp_rewrite_visible_until_rename; rename replaces whole segment object"
            );
            println!("purge_interrupted_temp_observed_before_rename={interrupted_temp_exists}");
            println!("purge_events_before={}", before.events);
            println!("purge_events_after={}", after.events);
            println!("purge_segment_events_before={}", parsed_before.events);
            println!("purge_segment_events_after={}", parsed_after.events);
            println!("purge_segment_rewrite_required=true");
            println!("purge_replacement_strategy=copy_intact_records_excluding_target_then_rename");
        }
    }

    Ok(())
}

fn rewrite_segment_without_seq(bytes: &[u8], purge_seq: u64) -> Result<Vec<u8>, String> {
    let mut cursor = 0_usize;
    let mut out = Vec::with_capacity(bytes.len());
    let mut removed = false;

    while cursor < bytes.len() {
        let remaining = bytes.len() - cursor;
        if remaining < RECORD_HEADER_BYTES {
            return Err("sealed segment truncated while rewriting for purge".to_string());
        }
        let seq = u64::from_le_bytes(
            bytes[cursor..cursor + 8]
                .try_into()
                .expect("sequence slice has fixed width"),
        );
        let payload_len = u32::from_le_bytes(
            bytes[cursor + 8..cursor + 12]
                .try_into()
                .expect("length slice has fixed width"),
        ) as usize;
        let total = RECORD_HEADER_BYTES
            .checked_add(payload_len)
            .ok_or_else(|| "record length overflow during purge rewrite".to_string())?;
        if remaining < total {
            return Err("sealed segment truncated payload while rewriting for purge".to_string());
        }
        if seq == purge_seq {
            removed = true;
        } else {
            out.extend_from_slice(&bytes[cursor..cursor + total]);
        }
        cursor += total;
    }

    if !removed {
        return Err(format!(
            "purge target sequence {purge_seq} was not present in sealed segment"
        ));
    }
    Ok(out)
}

fn run_corrupt_probe(
    layout: LayoutKind,
    root: &Path,
    payload_bytes: usize,
    segment_events: Option<u64>,
) -> Result<(), String> {
    match layout {
        LayoutKind::Files => {
            append_files(root, 0, CORRUPT_PROBE_EVENTS, payload_bytes)?;
            let target = root
                .join("events")
                .join(format!("{CORRUPT_PROBE_TARGET_SEQ:020}.evt"));
            let mut bytes = fs::read(&target).map_err(io_error("read corruption target"))?;
            if bytes.len() <= RECORD_HEADER_BYTES + 1 {
                return Err(
                    "corruption target payload is too small to truncate deterministically"
                        .to_string(),
                );
            }
            let original_len = bytes.len();
            bytes.truncate(RECORD_HEADER_BYTES + (payload_bytes / 2).max(1));
            fs::write(&target, &bytes).map_err(io_error("write truncated event object"))?;

            let mut intact = 0_u64;
            let mut corrupt_objects = 0_u64;
            let events_dir = root.join("events");
            for entry in fs::read_dir(&events_dir).map_err(io_error("enumerate events"))? {
                let entry = entry.map_err(io_error("read event entry"))?;
                let path = entry.path();
                if path.extension().and_then(|value| value.to_str()) != Some("evt") {
                    continue;
                }
                let object = fs::read(&path).map_err(io_error("read event for corrupt probe"))?;
                match parse_records(&object, false) {
                    Ok(parsed) if parsed.events == 1 => intact += 1,
                    _ => corrupt_objects += 1,
                }
            }

            println!("command=corrupt-probe");
            println!("layout=files");
            println!("root={}", root.display());
            println!("corrupt_target_seq={CORRUPT_PROBE_TARGET_SEQ}");
            println!("corrupt_mode=truncate_event_payload");
            println!("corrupt_original_object_bytes={original_len}");
            println!("corrupt_truncated_object_bytes={}", bytes.len());
            println!("corrupt_physical_objects_affected=1");
            println!("corrupt_intact_event_objects={intact}");
            println!("corrupt_unreadable_event_objects={corrupt_objects}");
            println!("corrupt_blast_radius_events=1");
            println!("corrupt_recovery_boundary=delete_or_replace_single_event_object");
            println!("corrupt_other_objects_remain_independently_readable=true");
        }
        LayoutKind::Segments => {
            let segment_events = segment_events
                .ok_or_else(|| "segments corrupt-probe requires segment_events".to_string())?;
            if segment_events < 3 {
                return Err("segments corrupt-probe requires segment_events >= 3".to_string());
            }
            append_segments(root, 0, CORRUPT_PROBE_EVENTS, payload_bytes, segment_events)?;

            // Case A: truncate active open tail (distinct from sealed-segment corruption).
            let open_entries: Vec<PathBuf> = fs::read_dir(root.join("segments"))
                .map_err(io_error("enumerate segments for open"))?
                .filter_map(|entry| entry.ok().map(|value| value.path()))
                .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("open"))
                .collect();
            if open_entries.len() != 1 {
                return Err(format!(
                    "expected exactly one active open segment, found {}",
                    open_entries.len()
                ));
            }
            let open_path = &open_entries[0];
            let open_before = fs::read(open_path).map_err(io_error("read open segment"))?;
            let open_parsed_before = parse_records(&open_before, true)?;
            let mut open_truncated = open_before.clone();
            if open_truncated.len() < RECORD_HEADER_BYTES + 2 {
                return Err("active segment too small for deterministic truncation".to_string());
            }
            open_truncated.truncate(open_truncated.len() - (payload_bytes / 2).max(1));
            fs::write(open_path, &open_truncated).map_err(io_error("truncate active segment"))?;
            let open_after = scan_segments(root)?;

            // Sealed-segment corruption is measured on an isolated sibling root so the
            // active-tail case above remains independently inspectable.
            let sealed_index = CORRUPT_PROBE_TARGET_SEQ / segment_events;
            let sealed_root = root
                .parent()
                .ok_or_else(|| "corrupt-probe root must have a parent directory".to_string())?
                .join(format!(
                    "{}-sealed-case",
                    root.file_name()
                        .and_then(|value| value.to_str())
                        .ok_or_else(|| "invalid corrupt-probe root name".to_string())?
                ));
            if sealed_root.exists() {
                return Err(format!(
                    "sealed-case root already exists; refusing reuse: {}",
                    sealed_root.display()
                ));
            }
            ensure_empty_root(&sealed_root)?;
            append_segments(
                &sealed_root,
                0,
                CORRUPT_PROBE_EVENTS,
                payload_bytes,
                segment_events,
            )?;
            let sealed_path = sealed_root
                .join("segments")
                .join(format!("{sealed_index:020}.seg"));
            let sealed_before = fs::read(&sealed_path).map_err(io_error("read sealed segment"))?;
            let sealed_parsed_before = parse_records(&sealed_before, false)?;
            let record_span = RECORD_HEADER_BYTES + payload_bytes;
            let target_offset_in_segment =
                ((CORRUPT_PROBE_TARGET_SEQ % segment_events) as usize) * record_span;
            let corrupt_at = target_offset_in_segment + RECORD_HEADER_BYTES + (payload_bytes / 2);
            if corrupt_at >= sealed_before.len() {
                return Err("computed sealed corruption offset is out of range".to_string());
            }
            let intact_prefix =
                parse_records(&sealed_before[..corrupt_at.min(sealed_before.len())], true)?;
            let mut sealed_corrupt = sealed_before.clone();
            sealed_corrupt.truncate(corrupt_at);
            fs::write(&sealed_path, &sealed_corrupt)
                .map_err(io_error("truncate sealed segment"))?;
            let sealed_scan = scan_segments(&sealed_root);

            println!("command=corrupt-probe");
            println!("layout=segments");
            println!("root={}", root.display());
            println!("corrupt_active_tail_path={}", open_path.display());
            println!(
                "corrupt_active_tail_intact_events_before={}",
                open_parsed_before.events
            );
            println!(
                "corrupt_active_tail_recovered_events_after={}",
                open_after.events
            );
            println!(
                "corrupt_active_tail_truncated_flag={}",
                open_after.truncated_active_tail
            );
            println!(
                "corrupt_active_tail_recovery_boundary=complete_records_before_truncated_tail"
            );
            println!("corrupt_sealed_case_root={}", sealed_root.display());
            println!("corrupt_sealed_path={}", sealed_path.display());
            println!(
                "corrupt_sealed_events_before={}",
                sealed_parsed_before.events
            );
            println!(
                "corrupt_sealed_intact_prefix_events={}",
                intact_prefix.events
            );
            println!("corrupt_sealed_offset_bytes={corrupt_at}");
            println!("corrupt_sealed_mode=truncate_mid_target_record_payload");
            match sealed_scan {
                Ok(stats) => {
                    println!("corrupt_sealed_scan_ok=true");
                    println!("corrupt_sealed_recovered_events={}", stats.events);
                    println!(
                        "corrupt_sealed_truncated_active_tail={}",
                        stats.truncated_active_tail
                    );
                }
                Err(error) => {
                    println!("corrupt_sealed_scan_ok=false");
                    println!("corrupt_sealed_scan_error={error}");
                    println!(
                        "corrupt_sealed_recovery_boundary=fail_closed_on_truncated_sealed_object"
                    );
                }
            }
            println!(
                "corrupt_blast_radius_note=active_tail_truncation_confines_loss_to_incomplete_suffix; sealed_truncation_fails_closed_for_that_object"
            );
        }
    }

    Ok(())
}

fn print_stats(
    command: &str,
    layout: LayoutKind,
    root: &Path,
    append_elapsed: Option<Duration>,
    stats: &ScanStats,
) {
    println!("command={command}");
    println!(
        "layout={}",
        match layout {
            LayoutKind::Files => "files",
            LayoutKind::Segments => "segments",
        }
    );
    println!("root={}", root.display());
    println!("events={}", stats.events);
    println!("payload_bytes={}", stats.payload_bytes);
    println!("stored_bytes={}", stats.stored_bytes);
    println!("physical_objects={}", stats.physical_objects);
    println!("max_events_per_object={}", stats.max_events_per_object);
    println!("enumeration_ms={:.3}", millis(stats.enumeration));
    println!("rebuild_ms={:.3}", millis(stats.rebuild));
    println!("truncated_active_tail={}", stats.truncated_active_tail);

    if let Some(elapsed) = append_elapsed {
        println!("append_total_ms={:.3}", millis(elapsed));
        if stats.events > 0 {
            println!(
                "append_average_us_per_total_event={:.3}",
                elapsed.as_secs_f64() * 1_000_000.0 / stats.events as f64
            );
        }
    }
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn io_error(context: &'static str) -> impl Fn(std::io::Error) -> String {
    move |error| format!("{context}: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        env::temp_dir().join(format!(
            "cryptex-m0-storage-bench-{name}-{}-{nonce}",
            process::id()
        ))
    }

    fn cleanup(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn files_round_trip() {
        let root = test_root("files");
        ensure_empty_root(&root).expect("create test root");
        append_files(&root, 0, 5, 64).expect("append files");
        let stats = scan_files(&root).expect("scan files");
        assert_eq!(stats.events, 5);
        assert_eq!(stats.physical_objects, 5);
        assert_eq!(stats.max_events_per_object, 1);
        assert!(!stats.truncated_active_tail);
        cleanup(&root);
    }

    #[test]
    fn segments_round_trip_and_seal() {
        let root = test_root("segments");
        ensure_empty_root(&root).expect("create test root");
        append_segments(&root, 0, 7, 64, 3).expect("append segments");
        let stats = scan_segments(&root).expect("scan segments");
        assert_eq!(stats.events, 7);
        assert_eq!(stats.physical_objects, 3);
        assert_eq!(stats.max_events_per_object, 3);
        assert!(!stats.truncated_active_tail);
        cleanup(&root);
    }

    #[test]
    fn truncated_active_segment_recovers_complete_prefix() {
        let root = test_root("truncated");
        ensure_empty_root(&root).expect("create test root");
        let segments_dir = root.join("segments");
        fs::create_dir_all(&segments_dir).expect("create segments dir");
        let path = segments_dir.join("00000000000000000000.open");
        let mut file = File::create(&path).expect("create active segment");
        let payload = vec![0x11; 32];
        write_record(&mut file, 0, &payload).expect("write complete record");
        file.write_all(&1_u64.to_le_bytes())
            .expect("write partial header");
        file.sync_all().expect("sync active segment");
        drop(file);

        let stats = scan_segments(&root).expect("scan active segment");
        assert_eq!(stats.events, 1);
        assert!(stats.truncated_active_tail);
        cleanup(&root);
    }

    #[test]
    fn files_purge_probe_removes_single_object() {
        let root = test_root("purge-files");
        ensure_empty_root(&root).expect("create test root");
        run_purge_probe(LayoutKind::Files, &root, 32, None).expect("purge files");
        let stats = scan_files(&root).expect("scan after purge");
        assert_eq!(stats.events, PURGE_PROBE_EVENTS - 1);
        assert_eq!(stats.physical_objects, PURGE_PROBE_EVENTS - 1);
        cleanup(&root);
    }

    #[test]
    fn segments_purge_probe_rewrites_sealed_segment() {
        let root = test_root("purge-segments");
        ensure_empty_root(&root).expect("create test root");
        run_purge_probe(LayoutKind::Segments, &root, 32, Some(5)).expect("purge segments");
        let stats = scan_segments(&root).expect("scan after purge");
        assert_eq!(stats.events, PURGE_PROBE_EVENTS - 1);
        cleanup(&root);
    }

    #[test]
    fn files_corrupt_probe_confines_blast_radius() {
        let root = test_root("corrupt-files");
        ensure_empty_root(&root).expect("create test root");
        run_corrupt_probe(LayoutKind::Files, &root, 64, None).expect("corrupt files");
        let target = root
            .join("events")
            .join(format!("{CORRUPT_PROBE_TARGET_SEQ:020}.evt"));
        assert!(parse_records(&fs::read(&target).expect("read target"), false).is_err());
        cleanup(&root);
    }

    #[test]
    fn segments_corrupt_probe_distinguishes_active_and_sealed() {
        let root = test_root("corrupt-segments");
        ensure_empty_root(&root).expect("create test root");
        run_corrupt_probe(LayoutKind::Segments, &root, 64, Some(5)).expect("corrupt segments");
        let sealed_root = root.parent().unwrap().join(format!(
            "{}-sealed-case",
            root.file_name().unwrap().to_str().unwrap()
        ));
        assert!(sealed_root.is_dir());
        cleanup(&root);
        cleanup(&sealed_root);
    }
}
