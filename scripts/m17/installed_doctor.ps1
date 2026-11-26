param(
    [Parameter(Mandatory)][int]$DesktopProcess,
    [Parameter(Mandatory)][string]$Expected,
    [Parameter(Mandatory)][string]$Contradiction
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$process = Get-Process -Id $DesktopProcess
$nameProperty = [System.Windows.Automation.AutomationElement]::NameProperty
$match = [System.Windows.Automation.PropertyCondition]::new($nameProperty, $Expected)
$conflict = [System.Windows.Automation.PropertyCondition]::new($nameProperty, $Contradiction)
$scope = [System.Windows.Automation.TreeScope]::Descendants
$timer = [Diagnostics.Stopwatch]::StartNew()
while ($timer.Elapsed.TotalSeconds -lt 35 -and -not $process.HasExited) {
    $process.Refresh()
    if ($process.MainWindowHandle -ne [IntPtr]::Zero) {
        $window = [System.Windows.Automation.AutomationElement]::FromHandle($process.MainWindowHandle)
        if ($window.Current.ProcessId -eq $DesktopProcess -and $window.FindFirst($scope, $match) -and
            -not $window.FindFirst($scope, $conflict)) { exit 0 }
    }
    Start-Sleep -Milliseconds 250
}
throw 'The installed Desktop did not render the expected Doctor status.'
