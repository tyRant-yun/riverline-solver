$ErrorActionPreference='Stop'
$output=Join-Path $PSScriptRoot '../../.artifacts/solver/m1-capacity'
$manifest=Get-Content (Join-Path $output 'frozen-manifest.json') -Raw | ConvertFrom-Json
$binary=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../engines/postflop/target/release/riverline-solver.exe'))
$runs=@()
foreach($case in $manifest.cases){
  $value=[ordered]@{schemaVersion=4}
  foreach($p in $case.PSObject.Properties){$value[$p.Name]=$p.Value}
  $value['limits']=$manifest.limits
  $inputPath=[IO.Path]::GetFullPath((Join-Path $output ($case.caseId+'.input.json')))
  $outputPath=[IO.Path]::GetFullPath((Join-Path $output ($case.caseId+'.json')))
  [IO.File]::WriteAllText($inputPath,($value|ConvertTo-Json -Depth 20))
  $start=[Diagnostics.ProcessStartInfo]::new($binary)
  $start.CreateNoWindow=$true;$start.UseShellExecute=$false;$start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true
  foreach($a in @('measure-turn',$inputPath,$outputPath)){$start.ArgumentList.Add($a)}
  $p=[Diagnostics.Process]::Start($start);$stdout=$p.StandardOutput.ReadToEndAsync();$stderr=$p.StandardError.ReadToEndAsync();$watch=[Diagnostics.Stopwatch]::StartNew();$peak=0;$killed=$false
  while(-not $p.WaitForExit(5)){
    $p.Refresh();try{$peak=[Math]::Max($peak,$p.PeakWorkingSet64)}catch{}
    if($watch.Elapsed.TotalSeconds -gt 130 -or $peak -gt 268435456){$p.Kill($true);$killed=$true;break}
  }
  $p.WaitForExit();$watch.Stop();$text=$stdout.GetAwaiter().GetResult();$errorText=$stderr.GetAwaiter().GetResult()
  $runs += [ordered]@{caseId=$case.caseId;wallMs=$watch.Elapsed.TotalMilliseconds;peakWorkingSetBytes=$peak;exitCode=$p.ExitCode;killed=$killed;error=$errorText;inputSha256=(Get-FileHash -LiteralPath $inputPath -Algorithm SHA256).Hash.ToLowerInvariant();binarySha256=(Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()}
  Write-Output "$($case.caseId) exit=$($p.ExitCode) ms=$($watch.ElapsedMilliseconds) peak=$peak"
  $p.Dispose()
  [IO.File]::WriteAllText((Join-Path $output 'execution.json'),([ordered]@{memoryMethod='PeakWorkingSet64 sampled every 5ms; includes computation and native/root serialization';os=[Environment]::OSVersion.ToString();cpu=(Get-CimInstance Win32_Processor).Name;threads=1;runs=$runs}|ConvertTo-Json -Depth 20))
}
