$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'collect-vm-check.ps1') -LibraryOnly
$root = Join-Path $env:LOCALAPPDATA ('Cryptex-Dev\vm-check-tests\' + [guid]::NewGuid().ToString('N'))
$pilot = Join-Path $root 'cx-m8-fixture'
$runtime = Join-Path $pilot 'desktop'
New-Item -ItemType Directory -Path "$runtime\bootstrap","$runtime\canonical\events\device" -Force | Out-Null
[IO.File]::WriteAllText("$runtime\pilot-runtime.marker", "m0-native-desktop-runtime-v1`n")
$journal = "$runtime\bootstrap\pending.json"
$event = "$runtime\canonical\events\device\segment.jsonl"
[IO.File]::WriteAllText($event, 'SECRET-EVENT-BODY')
[IO.File]::WriteAllText("$runtime\start.error", 'SECRET-LOG-BODY')
[IO.File]::WriteAllText("$runtime\canonical\events\device\writer.lock", '')
[IO.File]::WriteAllText("$runtime\canonical\events\device\writer_tip.json", '{}')
$tx = @{
    snapshot = @{ transaction_id = ('a' * 64) }
    state = 'anchored'
    transcript_messages = @('SECRET-SIGNED-MESSAGE','two','three','four')
    append_attempts = @(@{ secret = 'SECRET-ATTEMPT' })
}
[IO.File]::WriteAllText($journal, (@{ transactions = @($tx) } | ConvertTo-Json -Depth 8))
$before = (Get-FileHash -LiteralPath $event).Hash
$journalBefore = (Get-FileHash -LiteralPath $journal).Hash
$result = Get-PilotSummary $pilot
$serialized = $result | ConvertTo-Json -Depth 8
if ($result.status -ne 'observed' -or $result.transactions.Count -ne 1 -or
    $result.transactions[0].message_count -ne 4 -or $result.transactions[0].append_attempt_count -ne 1 -or
    $result.canonical_event_file_count -ne 1) { throw 'Valid pilot summary regression' }
if ($serialized -match 'SECRET|Users\\|CommandLine') { throw 'Private content leaked' }
if ($before -ne (Get-FileHash -LiteralPath $event).Hash -or
    $journalBefore -ne (Get-FileHash -LiteralPath $journal).Hash) { throw 'Pilot files changed' }
Write-Output 'PASS: summary counts, redaction, canonical and journal preservation'
$compact = @{
    schema_version = 3; transactions = @(); exchanges = @()
    terminal_receipts = @(
        @{ kind = 'anchored'; snapshot = $tx.snapshot; transcript_messages = $tx.transcript_messages
           verified_anchor = @{ attempt = @{ secret = 'SECRET-ATTEMPT' } } },
        @{ kind = 'cancelled'; snapshot = @{ transaction_id = ('b' * 64) } }
    )
}
[IO.File]::WriteAllText($journal, ($compact | ConvertTo-Json -Depth 12))
$compactBefore = (Get-FileHash -LiteralPath $journal).Hash
$result = Get-PilotSummary $pilot
if ($result.status -ne 'observed' -or $result.transactions.Count -ne 2 -or
    $result.transactions[0].state -ne 'anchored' -or $result.transactions[0].message_count -ne 4 -or
    $result.transactions[0].append_attempt_count -ne 1 -or $result.transactions[1].state -ne 'cancelled' -or
    $result.transactions[1].append_attempt_count -ne 0) { throw 'Compact receipt summary regression' }
if (($result | ConvertTo-Json -Depth 8) -match 'SECRET|Users\\|CommandLine') { throw 'Compact private data leaked' }
if ($compactBefore -ne (Get-FileHash -LiteralPath $journal).Hash) { throw 'Compact journal modified' }
Write-Output 'PASS: compact terminal receipts retain states and redact content'
[IO.File]::WriteAllText($journal, 'malformed SECRET-CORRUPTION')
$result = Get-PilotSummary $pilot
if ($result.status -ne 'read_failed' -or ($result | ConvertTo-Json) -match 'SECRET') { throw 'Invalid journal not safely reported' }
Write-Output 'PASS: malformed journal reported without content'
$tx.snapshot.transaction_id = 'SECRET-ID'
[IO.File]::WriteAllText($journal, (@{ transactions = @($tx) } | ConvertTo-Json -Depth 8))
$result = Get-PilotSummary $pilot
if ($result.error_code -ne 'invalid_transaction_id' -or $result.transactions.Count -ne 0) { throw 'Invalid ID exported' }
Write-Output 'PASS: unexpected ID rejected'
[IO.File]::WriteAllText($journal, '{}')
if ((Get-PilotSummary $pilot).error_code -ne 'invalid_journal') { throw 'Unknown schema accepted' }
Write-Output 'PASS: missing transaction schema rejected'
[IO.File]::WriteAllText($journal, (' ' * (8MB + 1)))
if ((Get-PilotSummary $pilot).error_code -ne 'size_limit') { throw 'Oversized journal accepted' }
Write-Output 'PASS: oversized journal refused'
[IO.File]::WriteAllText("$runtime\pilot-runtime.marker", 'other marker')
if ((Get-PilotSummary $pilot).error_code -ne 'invalid_marker') { throw 'Unmarked runtime accepted' }
Write-Output 'PASS: wrong marker refused'
# Junction regression requires no administrator privilege; never traverse its target.
$linked = Join-Path $root 'cx-m8-linked'
New-Item -ItemType Junction -Path $linked -Target $pilot | Out-Null
if ((Get-PilotSummary $linked).error_code -ne 'unsafe_path') { throw 'Junction followed' }
Write-Output 'PASS: reparse profile refused'
Write-Output 'PASS 8 groups. Synthetic fixtures retained under LocalAppData/Cryptex-Dev/vm-check-tests.'
