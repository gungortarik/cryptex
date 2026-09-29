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
}
