# M0 P0-1 — Initial Syncthing Scan/Sync Protocol

**Status:** IN PROGRESS  
**Scope:** P0-1 initial Syncthing scan/sync measurement only  
**Architecture effect:** none; this protocol does not select a production storage layout or complete P0-1/P0-4.

## Authority and requirement

`docs/23_REQUIRED_TECHNICAL_SPIKES.md` requires P0-1 to measure initial Syncthing scan/sync for the one-file-per-event and sealed-segment candidates at realistic scale. `docs/milestones/M0_FOUNDATION.md` requires the evidence to be reproducible from repository-controlled code/scripts and physically verified on macOS and Windows.

## Existing verified preconditions

Before this protocol starts:

- isolated Cryptex-Dev Syncthing homes exist on macOS and Windows
- both current benchmark processes use Syncthing v2.1.5
- GUI/API is bound only to `127.0.0.1:18384`
- sync listener is `tcp://0.0.0.0:22001`
- global discovery, local discovery, relay, NAT and automatic upgrades are disabled for the benchmark profiles
- peer addresses are explicit direct-LAN TCP addresses
- bidirectional direct TCP reachability is verified
- both peer configurations are verified and both runtime connections are established
- no benchmark folder existed before initial-sync preparation

These are P0-1 benchmark controls only, not production pairing semantics.

## Corpus preservation

The retained local-filesystem benchmark corpus is not used directly as a Syncthing folder.

For each initial-sync run, the repository preparation helper creates a new working copy under the runtime-only root:

```text
Cryptex-Dev/syncthing-p0-1-runs/<folder-id>/...
```

The retained source corpus under `Cryptex-Dev/benchmarks/p0-1/...` remains untouched. Copy time is setup cost and is explicitly outside the Syncthing timing window.

The helper refuses to delete or automatically reuse an existing work path.

## Folder roles

Each cross-platform run uses the same unique Syncthing folder ID on both peers.

Destination is prepared first:

- empty working directory
- folder type: `receiveonly`
- unpaused
- filesystem watcher disabled
- explicit intended peer only
- allowed to become idle before the timed run starts

Source is prepared second:

- working copy of the retained corpus
- folder type: `sendonly`
- paused
- filesystem watcher disabled
- explicit intended peer only
- source file count and logical byte count verified before Syncthing timing begins

The source remains paused until the timed runner starts. This removes human delay between separate-machine preparation commands from the measured interval.

## Timing definition

The timed runner executes on the source machine.

`t0` is taken immediately before the source folder is unpaused through the local Syncthing REST API.

Initial scan completion is the first observed state where:

- source `localFiles` equals the expected physical file count for the candidate
- source `localBytes` equals the expected stored logical byte count
- source folder state is no longer `scanning`

Initial synchronization completion is the first observation, after source scan completion, where Syncthing's peer completion endpoint reports:

- completion `>= 100`
- `needBytes = 0`
- `needItems = 0`
- `needDeletes = 0`
- `globalItems` equals the expected physical file count

The runner then pauses the source folder again to freeze the measured state.

Reported metrics include:

- UTC start / scan-complete / sync-complete timestamps
- initial source scan elapsed milliseconds
- total initial scan+sync elapsed milliseconds
- source local/global file and byte counts
- peer completion / need counts
- Syncthing version
- source and destination platform labels
- candidate layout and event count

## First controlled run

The first measurement is the 10k one-file-per-event candidate in this direction:

```text
source=macOS
source retained corpus=macos-10000-20260929T054105Z/data/files-10000
destination=Windows
layout=files
events=10000
expected physical files=10000
expected stored logical bytes=10360000
```

Working folder ID:

```text
p0-1-10k-files-macos-to-windows
```

### Windows destination preparation — VERIFIED

The guarded preparation helper was run on the physical Windows benchmark machine after the direct TCP peer connection checkpoint passed.

Observed output:

```text
home=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1
local_device_id=G3IQV46-V6WYJ2U-WDLQQOC-FXMJBGA-QRIXPNV-GVARBCC-IHYTOBI-BOIG6QA
peer_id=MHS264T-3YLCW4F-GL35TOB-DXZ6SUX-4MVY7YK-AZZFNZI-XFOXHN3-DYMMEQM
folder_id=p0-1-10k-files-macos-to-windows
folder_path=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1-runs\p0-1-10k-files-macos-to-windows\destination
role=destination
folder_type=receiveonly
fs_watcher_enabled=false
pre_config_files=0
pre_config_bytes=0
paused=false
peer_runtime_connected=true
scope=isolated_p0_1_initial_sync_preparation_only
```

This verifies that the Windows destination working folder was created empty, configured as `receiveonly`, left unpaused, and prepared while the intended macOS benchmark peer was connected. No source folder had yet been configured, so the timed sync had not started at this checkpoint.

### macOS source preparation

Pending physical execution and verification. The source working copy must contain exactly 10,000 files and 10,360,000 logical bytes and remain paused until the timed runner starts.

After source preparation is physically verified, the timed source runner may unpause the source and record initial scan and synchronization completion. After this run is physically verified and evidence is recorded, the equivalent 10k sealed-segment run will be performed under a distinct working folder ID. Reverse-direction and larger scale points remain required before P0-1 acceptance.

## Boundary

A successful Syncthing transport measurement does not prove Cryptex replication semantics or complete P0-4. Production pairing/device trust remains governed by the appropriate later milestone and accepted architecture.
