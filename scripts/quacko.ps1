if ($global:QuackoShellLoaded) { return }
$global:QuackoShellLoaded = $true
$global:QuackoHookDefault = '__QUACKO_HOOK__'
$global:QuackoLastHistoryId = (Get-History -Count 1).Id

$global:QuackoInteractive = @(
  'vi', 'vim', 'nvim', 'nano', 'emacs', 'micro', 'hx', 'less', 'more', 'man', 'top', 'htop', 'btop',
  'ssh', 'mosh', 'tmux', 'claude', 'cursor-agent', 'codex', 'aider', 'gemini', 'ipython', 'fzf',
  'lazygit', 'tig', 'notepad', 'edit', 'Get-Content'
)
$global:QuackoRepls = @(
  'python', 'python3', 'py', 'node', 'irb', 'psql', 'mysql', 'sqlite3', 'redis-cli', 'mongosh',
  'pwsh', 'powershell', 'cmd', 'bash', 'wsl'
)

function global:Get-QuackoDuration([int]$Seconds) {
  if ($Seconds -lt 60) { return "${Seconds}s" }
  if ($Seconds -lt 3600) { return "$([math]::Floor($Seconds / 60))m $($Seconds % 60)s" }
  return "$([math]::Floor($Seconds / 3600))h $([math]::Floor(($Seconds % 3600) / 60))m"
}

function global:Test-QuackoQuiet([string]$CommandLine) {
  $words = @($CommandLine.Trim() -split '\s+' | Where-Object { $_ -ne '' })
  while ($words.Count -gt 0 -and ($words[0] -match '^(sudo|time|env|&)$' -or $words[0] -match '^\$env:')) {
    $words = @($words | Select-Object -Skip 1)
  }
  if ($words.Count -eq 0) { return $true }
  $leaf = @($words[0].Trim("'", '"') -split '[\\/]')[-1]
  $program = [System.IO.Path]::GetFileNameWithoutExtension($leaf)
  if ($global:QuackoInteractive -contains $program) { return $true }
  $ignored = @("$env:QUACKO_IGNORE" -split '\s+' | Where-Object { $_ -ne '' })
  if ($ignored -contains $program) { return $true }
  if ($global:QuackoRepls -contains $program -and $words.Count -eq 1) { return $true }
  return $false
}

function global:Get-QuackoArgs($Entry, [bool]$Succeeded, [int]$ExitCode, [string]$Folder) {
  if ($null -eq $Entry -or [string]::IsNullOrWhiteSpace($Entry.CommandLine)) { return $null }
  $minimum = 30
  if ($env:QUACKO_MIN_SECS -match '^\d+$') { $minimum = [int]$env:QUACKO_MIN_SECS }
  $seconds = [int][math]::Floor(($Entry.EndExecutionTime - $Entry.StartExecutionTime).TotalSeconds)
  if ($seconds -lt $minimum) { return $null }
  if ($Entry.ExecutionStatus -eq 'Stopped' -or $ExitCode -eq -1073741510) { return $null }
  if (Test-QuackoQuiet $Entry.CommandLine) { return $null }

  $title = ($Entry.CommandLine -replace '[\r\n\t]+', ' ').Trim()
  if ($title.Length -gt 60) { $title = $title.Substring(0, 59) + [char]0x2026 }
  $took = Get-QuackoDuration $seconds
  $arguments = @('--from', 'terminal', '--source', 'custom', '--title', $title, '--project', $Folder)
  if ($Succeeded) {
    $arguments += @('--status', 'ok', '--message', $took)
  } else {
    $code = if ($ExitCode -ne 0) { $ExitCode } else { 1 }
    $arguments += @('--status', 'fail', '--message', "exit $code $([char]0x00B7) $took", '--sticky')
  }
  return , $arguments
}

$global:QuackoPreviousPrompt = $function:prompt

function global:prompt {
  $succeeded = $?
  $exitCode = $global:LASTEXITCODE
  try {
    $entry = Get-History -Count 1
    if ($entry -and $entry.Id -ne $global:QuackoLastHistoryId) {
      $global:QuackoLastHistoryId = $entry.Id
      $code = if ($succeeded) { 0 } elseif ($exitCode -is [int]) { $exitCode } else { 1 }
      $arguments = Get-QuackoArgs $entry $succeeded $code (Split-Path -Leaf (Get-Location).Path)
      $hook = if ($env:QUACKO_NOTIFY) { $env:QUACKO_NOTIFY } else { $global:QuackoHookDefault }
      if ($arguments -and (Test-Path -LiteralPath $hook)) {
        $null | & $hook @arguments 2>$null | Out-Null
      }
    }
  } catch {
  }
  $global:LASTEXITCODE = $exitCode
  & $global:QuackoPreviousPrompt
}
