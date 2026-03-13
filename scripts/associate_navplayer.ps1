param(
    [string]$ExePath = $(Join-Path $PSScriptRoot '..\target\debug\navplayer.exe')
)

$ErrorActionPreference = 'Stop'

try {
    $resolvedExe = (Resolve-Path $ExePath).Path
} catch {
    Write-Error "navplayer.exe not found: $ExePath"
    Write-Host 'Build the requested target first, or pass an existing exe path.'
    Write-Host 'Debug build example: cargo build'
    Write-Host 'Release build example: cargo build --release'
    exit 1
}

$command = '"{0}" "%1"' -f $resolvedExe
$icon = '"{0}",0' -f $resolvedExe

$progId = 'naVPlayer.video'
$extensions = @('.mp4', '.mov')

New-Item -Path "HKCU:\Software\Classes\$progId" -Force | Out-Null
Set-Item -Path "HKCU:\Software\Classes\$progId" -Value 'naVPlayer Video'

New-Item -Path "HKCU:\Software\Classes\$progId\DefaultIcon" -Force | Out-Null
Set-Item -Path "HKCU:\Software\Classes\$progId\DefaultIcon" -Value $icon

New-Item -Path "HKCU:\Software\Classes\$progId\shell\open\command" -Force | Out-Null
Set-Item -Path "HKCU:\Software\Classes\$progId\shell\open\command" -Value $command

foreach ($ext in $extensions) {
    New-Item -Path "HKCU:\Software\Classes\$ext" -Force | Out-Null
    Set-Item -Path "HKCU:\Software\Classes\$ext" -Value $progId
}

Write-Host "Associated .mp4 and .mov with $resolvedExe"
Write-Host 'You may need to re-open Explorer or confirm the app choice in Windows once.'
