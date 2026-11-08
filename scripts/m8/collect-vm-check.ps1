# Read-only pilot diagnostics. No daemon commands, publication or trust writes.
param(
    [switch] $LibraryOnly,
    [switch] $NoOpen
)

$ErrorActionPreference = 'Stop'

function Test-PlainPath {
    param([string] $Path)
    $item = Get-Item -LiteralPath $Path -Force -ErrorAction Stop
    while ($null -ne $item) {
        if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { return $false }
        if ($item -is [IO.DirectoryInfo]) { $item = $item.Parent } else { $item = $item.Directory }
    }
    return $true
}

function Read-BoundedJson {
    param([string] $Path)
    if (-not (Test-PlainPath $Path)) { throw 'unsafe_path' }
    $file = Get-Item -LiteralPath $Path
    if ($file.Length -gt 8MB) { throw 'size_limit' }
    return (Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json)
}

function Get-PilotSummary {
    param([string] $Root)
    $result = [ordered]@{
        profile = Split-Path $Root -Leaf
        status = 'unread'
        transactions = @()
        diagnostic_files_present = @()
        canonical_event_file_count = 0
    }
    try {
        if (-not (Test-PlainPath $Root)) { throw 'unsafe_path' }
        $runtime = Join-Path $Root 'desktop'
        $marker = Join-Path $runtime 'pilot-runtime.marker'
        if (-not (Test-PlainPath $marker)) { throw 'unsafe_path' }
        if ((Get-Item -LiteralPath $marker).Length -gt 128) { throw 'invalid_marker' }
        if ((Get-Content -LiteralPath $marker -Raw).Trim() -ne 'm0-native-desktop-runtime-v1') {
            throw 'invalid_marker'
        }
        $pending = Join-Path $runtime 'bootstrap\pending.json'
        if (Test-Path -LiteralPath $pending) {
            $journal = Read-BoundedJson $pending
            if ($null -eq $journal.transactions) { throw 'invalid_journal' }
            $transactions = @($journal.transactions)
            if ($transactions.Count -gt 1000) { throw 'size_limit' }
            $result.transactions = @($transactions | ForEach-Object {
                $tx = $_
                $txId = [string]$tx.snapshot.transaction_id
                if ($txId -notmatch '^[0-9a-f]{64}$') { throw 'invalid_transaction_id' }
                $state = [string]$tx.state
                if ($state -notmatch '^[a-z_]{1,64}$') { throw 'invalid_state' }
                [pscustomobject]@{
                    transaction_id = $txId
                    state = $state
                    message_count = @($tx.transcript_messages | Where-Object { $null -ne $_ }).Count
                    append_attempt_count = @($tx.append_attempts | Where-Object { $null -ne $_ }).Count
                }
            })
        }
        foreach ($name in @('start.error','window.error','restart.error','gui-proof.snapshot')) {
            if (Test-Path -LiteralPath (Join-Path $runtime $name) -PathType Leaf) {
                $result.diagnostic_files_present += $name
            }
        }
        # Inventory only, bounded depth, no event bodies or filenames exported.
        $events = Join-Path $runtime 'canonical\events'
        if (Test-Path -LiteralPath $events -PathType Container) {
            if (-not (Test-PlainPath $events)) { throw 'unsafe_path' }
            foreach ($device in @(Get-ChildItem -LiteralPath $events -Directory -Force)) {
                if (-not (Test-PlainPath $device.FullName)) { throw 'unsafe_path' }
                foreach ($file in @(Get-ChildItem -LiteralPath $device.FullName -File -Force)) {
                    if (-not (Test-PlainPath $file.FullName)) { throw 'unsafe_path' }
                    if ($file.Name -ne 'writer_tip.json' -and $file.Extension -ne '.lock') {
                        $result.canonical_event_file_count++
                    }
                }
            }
        }
        $result.status = 'observed'
    } catch {
        # Never export arbitrary exception text, journal content, or private paths.
        $result.status = 'read_failed'
        $result.transactions = @()
        $code = $_.Exception.Message
        if ($code -notin @('unsafe_path','size_limit','invalid_marker','invalid_journal','invalid_transaction_id','invalid_state')) {
            $code = 'unreadable_or_invalid_data'
        }
        $result.error_code = $code
    }
    return [pscustomobject]$result
}

function Get-VmCheckReport {
    $report = [ordered]@{
        schema_version = 1
        collected_utc = [DateTime]::UtcNow.ToString('o')
        scope = 'read_only_pilot_preflight_not_transfer_test'
        transfer_test = 'not_run'
        publication = 'not_activated_by_this_tool'
        powershell_version = $PSVersionTable.PSVersion.ToString()
        os_version = [Environment]::OSVersion.Version.ToString()
        collector_sha256 = (Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash
        processes = @()
        ipv4 = @()
        pilot_profiles = @()
        collection_errors = @()
    }
    try {
        $report.processes = @(Get-CimInstance Win32_Process -Filter "Name='cryptex-desktop.exe' OR Name='cryptexd.exe' OR Name='syncthing.exe'" | ForEach-Object {
            $process = $_
            $hash = $null
            if ($process.ExecutablePath) {
                try { $hash = (Get-FileHash -LiteralPath $process.ExecutablePath -Algorithm SHA256).Hash } catch { }
            }
            $pilotName = $null
            # Export only a pilot folder label, never the full command line or path.
            if ($process.CommandLine -match 'cx-m8-[a-zA-Z0-9-]+(?=\\desktop)') { $pilotName = $Matches[0] }
            [pscustomobject]@{
                name = $process.Name
                pid = $process.ProcessId
                parent_pid = $process.ParentProcessId
                executable_sha256 = $hash
                executable_hash_status = $(if ($hash) { 'observed' } else { 'unavailable' })
                pilot_profile = $pilotName
            }
        })
    } catch { $report.collection_errors += 'process_inventory_failed' }
    try {
        $report.ipv4 = @(Get-NetIPAddress -AddressFamily IPv4 -ErrorAction Stop |
            Where-Object { $_.IPAddress -ne '127.0.0.1' -and $_.IPAddress -notlike '169.254.*' } |
            Select-Object IPAddress, PrefixLength, InterfaceIndex, AddressState)
    } catch { $report.collection_errors += 'network_inventory_failed' }
    try {
        $candidates = @(Get-ChildItem -LiteralPath ([IO.Path]::GetTempPath()) -Directory -Filter 'cx-m8-*' -ErrorAction Stop |
            Where-Object { $_.Name -match '^cx-m8-[a-zA-Z0-9-]+$' })
        foreach ($candidate in $candidates) {
            if (-not (Test-PlainPath $candidate.FullName)) {
                $report.collection_errors += 'pilot_reparse_path_skipped'
                continue
            }
            if (Test-Path -LiteralPath (Join-Path $candidate.FullName 'desktop\pilot-runtime.marker')) {
                $report.pilot_profiles += Get-PilotSummary $candidate.FullName
            }
        }
    } catch { $report.collection_errors += 'pilot_inventory_failed' }
    return [pscustomobject]$report
}

if ($LibraryOnly) { return }

try {
    Write-Host 'Cryptex VM kontrolu basladi. Lutfen bu pencereyi kapatmayin.'
    $report = Get-VmCheckReport
    $destination = Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\vm-checks\' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $destination -Force | Out-Null
    $utf8 = New-Object Text.UTF8Encoding($false)
    [IO.File]::WriteAllText((Join-Path $destination 'report.json'), ($report | ConvertTo-Json -Depth 12), $utf8)
    $lines = @(
        'Cryptex VM durum raporu',
        '',
        'Bu paket durum toplar. Cihazlar arasi aktarim testi yapmaz.',
        'Mevcut profilleri, eslesmeyi, ag ayarlarini ve uygulamalari degistirmez.',
        'Not icerigi, anahtarlar, tam gunlukler ve imzali mesajlar rapora eklenmez.',
        '',
        ('Calisan ilgili surec sayisi: ' + $report.processes.Count),
        ('Bulunan pilot profil sayisi: ' + $report.pilot_profiles.Count),
        ('Toplama hatasi sayisi: ' + $report.collection_errors.Count)
    )
    foreach ($profile in $report.pilot_profiles) {
        $lines += ('Profil: ' + $profile.profile + ' / ' + $profile.status)
        foreach ($tx in $profile.transactions) {
            $lines += ('  Islem: ' + $tx.transaction_id + ' / ' + $tx.state + ' / mesaj: ' + $tx.message_count + ' / yazma denemesi: ' + $tx.append_attempt_count)
        }
    }
    $lines += @('', 'Bu rapor dosyalardaki durumu gozlemler; imza veya ag erisimi dogrulamasi degildir.',
        'Canli uygulama dosyalari degistiriyorsa sonuc anlik olabilir.',
        'SONUC.zip dosyasini sohbete ekleyin. Ekran goruntusu gerekmiyor.')
    [IO.File]::WriteAllText((Join-Path $destination 'OZET.txt'), ($lines -join "`r`n"), $utf8)
    $archive = Join-Path $destination 'SONUC.zip'
    Compress-Archive -LiteralPath (Join-Path $destination 'report.json'),(Join-Path $destination 'OZET.txt') -DestinationPath $archive
    Write-Host ('Rapor hazir: ' + $archive)
    if (-not $NoOpen) {
        Start-Process explorer.exe -ArgumentList ('/select,"{0}"' -f $archive)
    }
} catch {
    Write-Host 'Rapor olusturulamadi. Bu pencereyi kapatmadan hata kodunu paylasin.'
    Write-Host ('Hata turu: ' + $_.Exception.GetType().Name)
    exit 1
}
