$ErrorActionPreference = 'Stop'

$progId = 'naVPlayer.video'
$extensions = @('.mp4', '.mov')

foreach ($ext in $extensions) {
    $extPath = "HKCU:\Software\Classes\$ext"
    if (Test-Path $extPath) {
        $current = (Get-Item $extPath).GetValue('')
        if ($current -eq $progId) {
            Remove-Item -Path $extPath -Recurse -Force
        }
    }
}

$progIdPath = "HKCU:\Software\Classes\$progId"
if (Test-Path $progIdPath) {
    Remove-Item -Path $progIdPath -Recurse -Force
}

Write-Host 'Removed naVPlayer file association for .mp4 and .mov from HKCU.'
Write-Host 'You may need to re-open Explorer or choose a different default app in Windows.'
