param([int[]]$Sizes = @(16,64,128,1176), [int]$MaxSeconds = 8, [switch]$BothFlop, [string]$OutputDirectory = '.artifacts/af/flop-capacity')
$ErrorActionPreference = 'Stop'
$repoPath = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$targetPath = [IO.Path]::GetFullPath((Join-Path $repoPath $OutputDirectory))
if (-not $targetPath.StartsWith($repoPath + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Output must remain inside repository' }
[IO.Directory]::CreateDirectory($targetPath) | Out-Null
$binary = Join-Path $repoPath 'engines/postflop/target/release/riverline-solver.exe'
$board = @('2c','7d','Jh')
$deck = foreach ($r in '23456789TJQKA'.ToCharArray()) { foreach ($s in 'cdhs'.ToCharArray()) { "$r$s" } }
$available = @($deck | Where-Object { $_ -notin $board })
$combos = @(for ($i=0;$i -lt $available.Count;$i++) { for($j=$i+1;$j -lt $available.Count;$j++) { $available[$i]+$available[$j] } })
$measurements = @()
foreach ($size in $Sizes) {
  foreach ($deep in @($false,$true)) {
    if ($deep -and $size -gt 128) { continue }
    $ranges = @{}
    for ($k=0;$k -lt $size;$k++) { $ranges[$combos[[int][Math]::Floor($k*$combos.Count/$size)]] = 1 }
    $name = "flop-$size-" + $(if ($deep) {'three-street'} else {'flop-bet'})
    $value = [ordered]@{ schemaVersion=5;caseId=$name;board=$board;pot=200;stack=10000;ranges=@($ranges,$ranges);flopBets=$(if($BothFlop){@(100,100)}else{@(100,0)});turnBets=$(if($deep){@(100,100)}else{@(0,0)});riverBets=$(if($deep){@(100,100)}else{@(0,0)});limits=@{maxIterations=1000;targetExploitability=0.1;maxMemoryBytes=268435456;maxSeconds=$MaxSeconds} }
    $inputPath=Join-Path $targetPath "$name.input.json"
    [IO.File]::WriteAllText($inputPath,($value|ConvertTo-Json -Depth 12))
    foreach ($operation in @('preflight-flop','measure-flop')) {
      $outputPath=Join-Path $targetPath "$name.$operation.json"
      $info=[Diagnostics.ProcessStartInfo]::new($binary);$info.CreateNoWindow=$true;$info.UseShellExecute=$false;$info.RedirectStandardError=$true;$info.RedirectStandardOutput=$true
      foreach($argument in @($operation,$inputPath,$outputPath)) { $info.ArgumentList.Add($argument) }
      $proc=[Diagnostics.Process]::Start($info);$stderr=$proc.StandardError.ReadToEndAsync();$stdout=$proc.StandardOutput.ReadToEndAsync();$watch=[Diagnostics.Stopwatch]::StartNew();$peak=0;$killed=$false
      while(-not $proc.WaitForExit(10)) {
        $proc.Refresh();try {$peak=[Math]::Max($peak,$proc.PeakWorkingSet64)}catch{}
        if($watch.Elapsed.TotalSeconds -gt ($MaxSeconds + 10) -or $peak -gt 2147483648) {$proc.Kill($true);$killed=$true;break}
      }
      $proc.WaitForExit();$watch.Stop();$errorText=$stderr.GetAwaiter().GetResult();$null=$stdout.GetAwaiter().GetResult()
      $row=[ordered]@{caseId=$name;operation=$operation;combosPerPlayer=$size;wallMs=$watch.Elapsed.TotalMilliseconds;observedPeakWorkingSetBytes=$peak;exitCode=$proc.ExitCode;killed=$killed;error=$errorText.Trim();inputSha256=(Get-FileHash $inputPath -Algorithm SHA256).Hash.ToLowerInvariant()}
      if(Test-Path -LiteralPath $outputPath) { $result=Get-Content $outputPath -Raw|ConvertFrom-Json; if($operation -eq 'preflight-flop'){$row.estimatedBytes=$result.estimatedBytes}else{$row.iterations=$result.iterations;$row.elapsedMs=$result.elapsedMs;$row.stopReason=$result.stopReason} }
      if($peak -eq 0) { $row.observedPeakWorkingSetBytes=$null }
      $measurements+=$row;($row|ConvertTo-Json -Compress);$proc.Dispose()
      if($row.exitCode -ne 0 -or ($operation -eq 'preflight-flop' -and $result.estimatedBytes -gt 268435456)) { break }
    }
  }
}
$record=[ordered]@{date='2026-11-27';cpu=(Get-CimInstance Win32_Processor).Name;physicalMemoryBytes=(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory;threads=1;compression=$false;maxSeconds=$MaxSeconds;bothFlop=[bool]$BothFlop;note='Deterministic evenly-spread capacity ranges, not GTO data. Full 1176 board-legal combinations preserved. 256MiB application allocation gate; 2GiB process emergency stop; parent timeout is declared seconds +10.';runs=$measurements}
[IO.File]::WriteAllText((Join-Path $targetPath 'execution.json'),($record|ConvertTo-Json -Depth 12))
