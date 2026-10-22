param(
    [Parameter(Mandatory=$true)][string]$Binary,
    [Parameter(Mandatory=$true)][string]$FixtureRoot
)
$ErrorActionPreference = 'Stop'
$binaryItem = Get-Item -LiteralPath $Binary -ErrorAction Stop
if ($binaryItem -isnot [IO.FileInfo]) { throw 'Native binary must be an existing filesystem file' }
$Binary = $binaryItem.FullName
$expected = '36A0F7BC372F64FA7CC4F5654FA324C0DD9F7FEF2E07565E00C6E1CF73F50344'
if ((Get-FileHash -LiteralPath $Binary -Algorithm SHA256).Hash -ne $expected) { throw 'Pinned native binary mismatch' }
$fixturePath = [IO.Path]::GetFullPath($FixtureRoot)
$allowedParent = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'Cryptex-Dev\m8-verification'))
if (-not $fixturePath.StartsWith($allowedParent + '\',[StringComparison]::OrdinalIgnoreCase) -or -not ([IO.Path]::GetFileName($fixturePath)).StartsWith('m8-owned-native-') -or (Test-Path -LiteralPath $fixturePath)) { throw 'Fresh app-data synthetic fixture required' }
New-Item -ItemType Directory -Path $fixturePath | Out-Null
# Protect only this brand-new fixture before any native key/config is generated.
$sid = [Security.Principal.WindowsIdentity]::GetCurrent().User
$acl = New-Object Security.AccessControl.DirectorySecurity
$acl.SetOwner($sid)
$acl.SetAccessRuleProtection($true,$false)
$rule = New-Object Security.AccessControl.FileSystemAccessRule($sid,'FullControl','ContainerInherit,ObjectInherit','None','Allow')
$acl.AddAccessRule($rule)
Set-Acl -LiteralPath $fixturePath -AclObject $acl
$observed = Get-Acl -LiteralPath $fixturePath
$rules = @($observed.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier]))
if (-not $observed.AreAccessRulesProtected -or $rules.Count -ne 1 -or $rules[0].IdentityReference.Value -ne $sid.Value -or $rules[0].AccessControlType -ne 'Allow') { throw 'Private fixture ACL was not established' }
$profile = Join-Path $fixturePath 'profile'
New-Item -ItemType Directory -Path $profile | Out-Null
$start = New-Object Diagnostics.ProcessStartInfo
$start.FileName = $Binary
$start.Arguments = 'generate --home "' + $profile + '" --no-port-probing'
$start.UseShellExecute = $false
$start.CreateNoWindow = $true
$start.RedirectStandardOutput = $true
$start.RedirectStandardError = $true
$start.EnvironmentVariables.Clear()
foreach ($name in @('SYSTEMROOT','WINDIR','TEMP','TMP','LOCALAPPDATA','APPDATA','USERPROFILE')) {
    $value = [Environment]::GetEnvironmentVariable($name)
    if ($null -ne $value) { $start.EnvironmentVariables[$name] = $value }
}
$start.EnvironmentVariables['STHOMEDIR'] = $profile
$start.EnvironmentVariables['STNODEFAULTFOLDER'] = '1'
$process = [Diagnostics.Process]::Start($start)
$stdout = $process.StandardOutput.ReadToEndAsync()
$stderr = $process.StandardError.ReadToEndAsync()
if (-not $process.WaitForExit(15000)) {
    $process.Kill()
    throw 'Native fixture generation timed out'
}
$utf8 = New-Object Text.UTF8Encoding($false)
[IO.File]::WriteAllText((Join-Path $fixturePath 'generate.stdout.txt'),$stdout.Result,$utf8)
[IO.File]::WriteAllText((Join-Path $fixturePath 'generate.stderr.txt'),$stderr.Result,$utf8)
if ($process.ExitCode -ne 0) { throw ('Native fixture generation failed: ' + $process.ExitCode) }
$process.Dispose()
function New-LoopbackPort {
    $listener = New-Object Net.Sockets.TcpListener([Net.IPAddress]::Loopback,0)
    $listener.Start()
    try { return $listener.LocalEndpoint.Port } finally { $listener.Stop() }
}
$apiPort = New-LoopbackPort
$syncPort = New-LoopbackPort
while ($syncPort -eq $apiPort) { $syncPort = New-LoopbackPort }
$configPath = Join-Path $profile 'config.xml'
if ((Get-Item -LiteralPath $configPath).Length -gt 262144) { throw 'Oversized generated config' }
$settings = New-Object Xml.XmlReaderSettings
$settings.DtdProcessing = [Xml.DtdProcessing]::Prohibit
$settings.XmlResolver = $null
$reader = [Xml.XmlReader]::Create($configPath,$settings)
$xml = New-Object Xml.XmlDocument
$xml.XmlResolver = $null
try { $xml.Load($reader) } finally { $reader.Dispose() }
$gui = @($xml.SelectNodes('/configuration/gui'))
$options = @($xml.SelectNodes('/configuration/options'))
if ($gui.Count -ne 1 -or $options.Count -ne 1) { throw 'Invalid generated config shape' }
foreach ($folder in @($xml.SelectNodes('/configuration/folder'))) { [void]$folder.ParentNode.RemoveChild($folder) }
$address = @($gui[0].SelectNodes('address'))
if ($address.Count -ne 1) { throw 'Unique generated API address required' }
$address[0].InnerText = '127.0.0.1:' + $apiPort
$gui[0].SetAttribute('enabled','true')
$gui[0].SetAttribute('tls','false')
$values = @{ listenAddress=('tcp://127.0.0.1:' + $syncPort); globalAnnounceEnabled='false'; localAnnounceEnabled='false'; relaysEnabled='false'; natEnabled='false'; autoUpgradeIntervalH='0'; urAccepted='-1' }
foreach ($name in $values.Keys) {
    $nodes = @($options[0].SelectNodes($name))
    if ($nodes.Count -eq 0) { throw ('Missing generated isolation field: ' + $name) }
    $nodes[0].InnerText = $values[$name]
    for ($i=1; $i -lt $nodes.Count; $i++) { [void]$options[0].RemoveChild($nodes[$i]) }
}
$writerSettings = New-Object Xml.XmlWriterSettings
$writerSettings.Encoding = $utf8
$writerSettings.Indent = $true
$writerSettings.NewLineChars = "`n"
$writer = [Xml.XmlWriter]::Create($configPath,$writerSettings)
try { $xml.Save($writer) } finally { $writer.Dispose() }
$devices = @($xml.SelectNodes('/configuration/device'))
if ($devices.Count -ne 1) { throw 'Unique generated native self identity required' }
$fixture = [ordered]@{ schema_version=1; scope='m8-owned-native-synthetic'; binary=[IO.Path]::GetFullPath($Binary); api_port=$apiPort; native_id=$devices[0].GetAttribute('id') }
[IO.File]::WriteAllText((Join-Path $fixturePath 'fixture.json'),(($fixture | ConvertTo-Json) + "`n"),$utf8)
Write-Output 'Fresh protected loopback-only native fixture prepared. No transport was launched.'
