<#
.SYNOPSIS
  Замер A/V-синхронизации записи Capture Studio эталонным генератором syncgen.

.DESCRIPTION
  Ручной режим (по умолчанию): скрипт запускает генератор, вы сами начинаете и останавливаете
  запись в Capture Studio (окно генератора должно попадать в захватываемую область), затем
  скрипт анализирует самый свежий файл в папке записей.

  Автоматический режим (-Auto): скрипт сам запускает Capture Studio с указанной папкой данных
  и нажимает кнопку записи (координаты в логических пикселях окна: -ClickX/-ClickY), делает
  -Runs записей при одном запуске генератора и анализирует каждую.

  Требуется: собранные `capture-studio.exe` и `syncgen.exe` (cargo build --release),
  Python с numpy, ffmpeg/ffprobe в PATH или в переменной FFMPEG_DIR.

.EXAMPLE
  .\measure.ps1 -RecDir "$env:USERPROFILE\Videos"
.EXAMPLE
  .\measure.ps1 -Auto -DataDir D:\cs-test-data -RecDir D:\cs-test-data\rec -Runs 3
#>
param(
    [Parameter(Mandatory)] [string]$RecDir,
    [int]$Seconds = 30,
    [switch]$Auto,
    [string]$DataDir,
    [int]$Runs = 3,
    [int]$ClickX = 1140,
    [int]$ClickY = 684,
    [int]$GenX = 200,
    [int]$GenY = 200
)
$ErrorActionPreference = "Stop"
$Root = Resolve-Path "$PSScriptRoot\..\..\.."
$Syncgen = "$Root\target\release\syncgen.exe"
$App = "$Root\target\release\capture-studio.exe"
$Events = Join-Path ([System.IO.Path]::GetTempPath()) "syncgen-events.json"
# Вывод генератора: итог и предупреждения (пропущенные события, паузы цикла и звука).
$GenOut = Join-Path ([System.IO.Path]::GetTempPath()) "syncgen-out.txt"
$GenErr = Join-Path ([System.IO.Path]::GetTempPath()) "syncgen-err.txt"

function GenReport {
    if (Test-Path $GenOut) { Get-Content $GenOut | ForEach-Object { "генератор: $_" } }
    if ((Test-Path $GenErr) -and (Get-Item $GenErr).Length -gt 0) { Get-Content $GenErr | ForEach-Object { "генератор: $_" } }
}
$env:PYTHONIOENCODING = "utf-8"

function Analyze($file) {
    python "$PSScriptRoot\analyze.py" $file $Events
}

function Newest { Get-ChildItem "$RecDir\*.mp4", "$RecDir\*.mkv" -ErrorAction SilentlyContinue | Sort-Object LastWriteTime | Select-Object -Last 1 }

if (-not $Auto) {
    $before = Newest
    Write-Host "Генератор: $Seconds с. Начните запись в Capture Studio сейчас и остановите после того, как окно генератора закроется."
    & $Syncgen --seconds $Seconds --x $GenX --y $GenY --out $Events
    Read-Host "Остановите запись, дождитесь сохранения файла и нажмите Enter"
    $f = Newest
    if (-not $f -or ($before -and $f.FullName -eq $before.FullName -and $f.LastWriteTime -eq $before.LastWriteTime)) {
        throw "Новая запись в $RecDir не найдена"
    }
    Analyze $f.FullName
    return
}

if (-not $DataDir) { throw "Для -Auto нужен -DataDir (отдельная папка данных Capture Studio)" }
# Capture Studio допускает один экземпляр: при открытой программе тестовая копия не запустится.
if (Get-Process capture-studio -ErrorAction SilentlyContinue) { throw "Закройте Capture Studio перед автоматическим замером" }
$start = Get-Date
$p = Start-Process -FilePath $App -ArgumentList "--data-dir", $DataDir -PassThru
Start-Sleep -Seconds 3
if (-not ("SyncgenInput" -as [type])) {
    Add-Type @"
using System; using System.Runtime.InteropServices;
public class SyncgenInput { [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint m, IntPtr w, IntPtr l); }
"@
}
$h = $p.MainWindowHandle
function Click {
    $l = [IntPtr](($ClickY -shl 16) -bor ($ClickX -band 0xFFFF))
    [SyncgenInput]::PostMessageW($h, 0x200, [IntPtr]0, $l) | Out-Null; Start-Sleep -Milliseconds 80
    [SyncgenInput]::PostMessageW($h, 0x201, [IntPtr]1, $l) | Out-Null; Start-Sleep -Milliseconds 60
    [SyncgenInput]::PostMessageW($h, 0x202, [IntPtr]0, $l) | Out-Null
}
# Один запуск генератора на все записи: у каждого запуска генератора своя погрешность в
# несколько миллисекунд, так записи сравниваются между собой честно.
$g = Start-Process -FilePath $Syncgen -ArgumentList "--seconds", ($Runs * 22 + 6), "--x", $GenX, "--y", $GenY, "--out", $Events -PassThru `
    -RedirectStandardOutput $GenOut -RedirectStandardError $GenErr
Start-Sleep -Seconds 3
foreach ($r in 1..$Runs) {
    Click; Start-Sleep -Seconds 17
    Click; Start-Sleep -Seconds 5
}
$g.WaitForExit(30000) | Out-Null
$p.CloseMainWindow() | Out-Null; $p.WaitForExit(6000) | Out-Null
GenReport
# Из пары MKV + MP4 (ремукс) анализируется MP4.
Get-ChildItem "$RecDir\*.mp4", "$RecDir\*.mkv" | Where-Object { $_.LastWriteTime -gt $start } |
    Group-Object BaseName | Sort-Object Name | ForEach-Object {
        $f = $_.Group | Sort-Object { $_.Extension -ne ".mp4" } | Select-Object -First 1
        Write-Host "== $($f.Name)"; Analyze $f.FullName
    }
