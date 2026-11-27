param(
    [string]$OutputDirectory = '.artifacts/solver/r1',
    [ValidateSet('river-cases.json', 'river-tree-cases.json', 'turn-cases.json')][string]$Manifest = 'river-cases.json',
    [ValidateSet('validate-river', 'validate-river-tree', 'validate-turn')][string]$Command = 'validate-river'
)
$ErrorActionPreference = 'Stop'
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$output = [IO.Path]::GetFullPath((Join-Path $repository $OutputDirectory))
if (-not $output.StartsWith($repository + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Output must stay inside repository' }
[IO.Directory]::CreateDirectory($output) | Out-Null
$caseManifest = Get-Content (Join-Path $PSScriptRoot $Manifest) -Raw | ConvertFrom-Json
$binary = Join-Path $repository 'engines/postflop/target/release/riverline-solver.exe'
if (-not (Test-Path -LiteralPath $binary)) { throw 'Build the pinned solver first' }
$measurements = @()
foreach ($case in $caseManifest.cases) {
    $inputPath = Join-Path $output ($case.caseId + '.input.json')
    $suffix = if ($Command -eq 'validate-turn') { '.json' } else { '.output.json' }
    $outputPath = Join-Path $output ($case.caseId + $suffix)
    $inputValue = [ordered]@{ schemaVersion = $caseManifest.schemaVersion }
    foreach ($property in $case.PSObject.Properties) { $inputValue[$property.Name] = $property.Value }
    $inputValue['limits'] = $caseManifest.limits
    [IO.File]::WriteAllText($inputPath, ($inputValue | ConvertTo-Json -Depth 20))
    $start = [Diagnostics.ProcessStartInfo]::new($binary)
    $start.CreateNoWindow = $true
    $start.UseShellExecute = $false
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    foreach ($argument in @($Command, $inputPath, $outputPath)) { $start.ArgumentList.Add($argument) }
    $process = [Diagnostics.Process]::Start($start)
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $peak = $null
    while (-not $process.WaitForExit(5)) {
        $process.Refresh()
        try { $sample = $process.PeakWorkingSet64; if ($sample -gt 0 -and ($null -eq $peak -or $sample -gt $peak)) { $peak = $sample } } catch { }
        if ($watch.Elapsed.TotalSeconds -gt ($caseManifest.limits.maxSeconds + 10) -or ($null -ne $peak -and $peak -gt $caseManifest.limits.maxMemoryBytes)) { $process.Kill($true); throw 'Process exceeded declared benchmark time/RSS budget' }
    }
    $watch.Stop()
    $text = $stdout.GetAwaiter().GetResult()
    $errorText = $stderr.GetAwaiter().GetResult()
    if ($process.ExitCode -ne 0) { throw "$($case.caseId): $errorText" }
    $measurements += [ordered]@{ caseId = $case.caseId; wallMs = $watch.Elapsed.TotalMilliseconds; observedPeakWorkingSetBytes = $peak; binarySha256 = (Get-FileHash $binary -Algorithm SHA256).Hash.ToLowerInvariant(); inputSha256 = (Get-FileHash $inputPath -Algorithm SHA256).Hash.ToLowerInvariant(); outputSha256 = (Get-FileHash $outputPath -Algorithm SHA256).Hash.ToLowerInvariant(); stdout = $text.Trim() }
    $text.Trim()
    $process.Dispose()
}
$record = [ordered]@{ os = [Environment]::OSVersion.ToString(); cpu = (Get-CimInstance Win32_Processor).Name; physicalMemoryBytes = (Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory; threads = 1; compression = $false; memoryMethod = 'Windows process PeakWorkingSet64 polled every 5ms while alive; null if unobserved; one solver process incl serialization/query'; runs = $measurements }
[IO.File]::WriteAllText((Join-Path $output 'execution.json'), ($record | ConvertTo-Json -Depth 20))
