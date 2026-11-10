$ErrorActionPreference = 'Stop'
$script:pass = 0
$script:fail = 0
function Check([string]$Name, [bool]$Condition, $Detail = '') {
  if ($Condition) { $script:pass++ } else { $script:fail++; Write-Host "FALHOU (PowerShell): $Name $Detail" }
}

$work = Join-Path ([System.IO.Path]::GetTempPath()) ("quacko-ps-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $work | Out-Null
try {
  $log = Join-Path $work 'notify.log'
  $fake = Join-Path $work 'notify.ps1'
  Set-Content -LiteralPath $fake -Value "(`$args -join '|') | Add-Content -LiteralPath '$log'"
  $env:QUACKO_NOTIFY = $fake
  $env:QUACKO_MIN_SECS = '0'
  $env:QUACKO_IGNORE = 'meutool'

  function global:prompt { 'PS> ' }
  . (Join-Path $PSScriptRoot '..' 'quacko.ps1')

  function Entry([string]$Line, [int]$Seconds, [string]$Status = 'Completed') {
    $start = Get-Date '2026-01-01T10:00:00'
    [pscustomobject]@{
      CommandLine        = $Line
      ExecutionStatus    = $Status
      StartExecutionTime = $start
      EndExecutionTime   = $start.AddSeconds($Seconds)
    }
  }

  $a = Get-QuackoArgs (Entry 'cargo build --release' 134) $false 101 'lagoa-api'
  $text = $a -join '|'
  Check 'falha com código, tempo e --sticky' ($text -eq "--from|terminal|--source|custom|--title|cargo build --release|--project|lagoa-api|--status|fail|--message|exit 101 $([char]0x00B7) 2m 14s|--sticky") $text
  $a = Get-QuackoArgs (Entry 'npm test' 45) $true 0 'app'
  Check 'sucesso some sozinho' (($a -join '|') -eq '--from|terminal|--source|custom|--title|npm test|--project|app|--status|ok|--message|45s') ($a -join '|')
  $a = Get-QuackoArgs (Entry 'Write-Error x' 3) $false 0 'app'
  Check 'falha de cmdlet sem código vira exit 1' (($a -join '|') -like '*exit 1 *') ($a -join '|')

  $env:QUACKO_MIN_SECS = '30'
  Check 'comando rápido não avisa' ($null -eq (Get-QuackoArgs (Entry 'npm test' 5) $true 0 'app'))
  Check 'QUACKO_MIN_SECS padrão 30' ($null -ne (Get-QuackoArgs (Entry 'npm test' 30) $true 0 'app'))
  $env:QUACKO_MIN_SECS = '0'
  Check 'Ctrl+C não avisa' ($null -eq (Get-QuackoArgs (Entry 'npm test' 9 'Stopped') $false 0 'app'))
  Check 'Ctrl+C em programa nativo não avisa' ($null -eq (Get-QuackoArgs (Entry 'npm test' 9) $false -1073741510 'app'))
  Check 'programa interativo não avisa' ($null -eq (Get-QuackoArgs (Entry 'ssh servidor' 90) $true 0 'app'))
  Check 'caminho e extensão não enganam' ($null -eq (Get-QuackoArgs (Entry 'C:\Tools\vim.exe notas.txt' 90) $true 0 'app'))
  Check 'QUACKO_IGNORE' ($null -eq (Get-QuackoArgs (Entry 'meutool --x' 90) $true 0 'app'))
  Check 'REPL sem argumentos não avisa' ($null -eq (Get-QuackoArgs (Entry 'python' 90) $true 0 'app'))
  Check 'script do python avisa' ($null -ne (Get-QuackoArgs (Entry 'python treino.py' 90) $true 0 'app'))
  Check 'linha vazia não avisa' ($null -eq (Get-QuackoArgs (Entry '   ' 90) $true 0 'app'))
  $long = Get-QuackoArgs (Entry ('x' * 80) 90) $true 0 'app'
  Check 'título longo cortado em 60 com …' ($long[5].Length -eq 60 -and $long[5].EndsWith([string][char]0x2026)) $long[5]
  Check 'tempo em h m' ((Get-QuackoDuration 3780) -eq '1h 3m')

  [pscustomobject]@{
    CommandLine        = 'cargo build'
    ExecutionStatus    = 'Completed'
    StartExecutionTime = (Get-Date).AddSeconds(-5)
    EndExecutionTime   = Get-Date
  } | Add-History
  Write-Error 'falhou' -ErrorAction SilentlyContinue
  $shown = prompt
  Check 'prompt chama o hook e devolve o prompt anterior' ($shown -eq 'PS> ' -and (Test-Path -LiteralPath $log)) $shown
  if (Test-Path -LiteralPath $log) {
    $line = Get-Content -LiteralPath $log -Raw
    Check 'prompt manda o comando e a falha' ($line -like '*--title|cargo build|*--status|fail|*') $line
    Remove-Item -LiteralPath $log
  }
  $null = prompt
  Check 'mesmo comando não avisa de novo' (-not (Test-Path -LiteralPath $log))
} finally {
  Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "$($script:pass) ok, $($script:fail) falhas"
if ($script:fail -ne 0) { exit 1 }
