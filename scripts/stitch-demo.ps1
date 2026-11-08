<#
.SYNOPSIS
  Tailscale KVM Dual-PC Demo Recorder & Side-by-Side Stitcher (Windows PowerShell)

.EXAMPLE
  # Synchronized live capture across Windows Host + Linux/macOS Receiver via Host Web GUI API:
  .\scripts\stitch-demo.ps1 -Slot right -Duration 10

.EXAMPLE
  # Offline stitch of any two captured clips side-by-side:
  .\scripts\stitch-demo.ps1 -HostClip .\host.mp4 -RecvClip .\recv.mp4 -Layout right
#>
param(
    [string]$HostClip = "",
    [string]$RecvClip = "",
    [string]$Layout = "right",
    [int]$Duration = 0,
    [string]$ApiUrl = "http://127.0.0.1:9877",
    [string]$HostLabel = "WINDOWS HOST",
    [string]$RecvLabel = "LINUX RECEIVER",
    [string]$OutWebp = ""
)

$ErrorActionPreference = "Stop"
$RepoDir = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
if ([string]::IsNullOrWhiteSpace($OutWebp)) {
    $OutWebp = Join-Path $RepoDir "demoPics\02-dual-screen-demo.webp"
}

if (-not [string]::IsNullOrWhiteSpace($HostClip) -and -not [string]::IsNullOrWhiteSpace($RecvClip)) {
    $ffmpeg = Get-Command ffmpeg -ErrorAction SilentlyContinue
    if (-not $ffmpeg) {
        throw "ffmpeg is required in PATH for offline stitching."
    }
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $OutWebp) | Out-Null
    $OutMp4 = [System.IO.Path]::ChangeExtension($OutWebp, ".mp4")
    $OutGif = [System.IO.Path]::ChangeExtension($OutWebp, ".gif")

    $escH = $HostLabel.Replace("\", "\\").Replace(":", "\:").Replace("'", "\'")
    $escR = $RecvLabel.Replace("\", "\\").Replace(":", "\:").Replace("'", "\'")

    switch ($Layout.ToLowerInvariant()) {
        "left"   { $a=$RecvClip; $b=$HostClip; $la=$escR; $lb=$escH; $stack="hstack=inputs=2:shortest=1"; $pad="[v0]pad=iw+6:ih:0:0:color=#00d4be@0.35[v0p];"; $pw=720; $ph=450 }
        "up"     { $a=$RecvClip; $b=$HostClip; $la=$escR; $lb=$escH; $stack="vstack=inputs=2:shortest=1"; $pad="[v0]pad=iw:ih+6:0:0:color=#00d4be@0.35[v0p];"; $pw=960; $ph=450 }
        "top"    { $a=$RecvClip; $b=$HostClip; $la=$escR; $lb=$escH; $stack="vstack=inputs=2:shortest=1"; $pad="[v0]pad=iw:ih+6:0:0:color=#00d4be@0.35[v0p];"; $pw=960; $ph=450 }
        "down"   { $a=$HostClip; $b=$RecvClip; $la=$escH; $lb=$escR; $stack="vstack=inputs=2:shortest=1"; $pad="[v0]pad=iw:ih+6:0:0:color=#00d4be@0.35[v0p];"; $pw=960; $ph=450 }
        "bottom" { $a=$HostClip; $b=$RecvClip; $la=$escH; $lb=$escR; $stack="vstack=inputs=2:shortest=1"; $pad="[v0]pad=iw:ih+6:0:0:color=#00d4be@0.35[v0p];"; $pw=960; $ph=450 }
        default  { $a=$HostClip; $b=$RecvClip; $la=$escH; $lb=$escR; $stack="hstack=inputs=2:shortest=1"; $pad="[v0]pad=iw+6:ih:0:0:color=#00d4be@0.35[v0p];"; $pw=720; $ph=450 }
    }

    $p0 = "[0:v]setpts=PTS-STARTPTS,fps=15,scale=${pw}:${ph}:force_original_aspect_ratio=decrease,pad=${pw}:${ph}:(ow-iw)/2:(oh-ih)/2:color=#0b0f17,setsar=1"
    $p1 = "[1:v]setpts=PTS-STARTPTS,fps=15,scale=${pw}:${ph}:force_original_aspect_ratio=decrease,pad=${pw}:${ph}:(ow-iw)/2:(oh-ih)/2:color=#0b0f17,setsar=1"
    $fltWebp = "${p0},drawtext=text='${la}':x=14:y=12:fontsize=18:fontcolor=#f0f6fc:box=1:boxcolor=#0b0f17@0.82:boxborderw=7[v0];${p1},drawtext=text='${lb}':x=14:y=12:fontsize=18:fontcolor=#00d4be:box=1:boxcolor=#0b0f17@0.82:boxborderw=7[v1];${pad}[v0p][v1]${stack}[outv]"

    Write-Host "[*] Stitching $a + $b ($Layout) -> $OutWebp" -ForegroundColor Cyan
    & ffmpeg -y -i $a -i $b -filter_complex $fltWebp -map "[outv]" -c:v libwebp_anim -lossless 0 -compression_level 4 -q:v 78 -loop 0 -an $OutWebp
    & ffmpeg -y -i $a -i $b -filter_complex $fltWebp -map "[outv]" -c:v libx264 -preset veryfast -crf 22 -pix_fmt yuv420p -an $OutMp4

    $g0 = "[0:v]setpts=PTS-STARTPTS,fps=12,scale=600:376:force_original_aspect_ratio=decrease,pad=600:376:(ow-iw)/2:(oh-ih)/2:color=#0b0f17,setsar=1[v0]"
    $g1 = "[1:v]setpts=PTS-STARTPTS,fps=12,scale=600:376:force_original_aspect_ratio=decrease,pad=600:376:(ow-iw)/2:(oh-ih)/2:color=#0b0f17,setsar=1[v1]"
    $fltGif = "${g0};${g1};${pad}[v0p][v1]${stack}[outv];[outv]split[s0][s1];[s0]palettegen=max_colors=192:stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=3[gifv]"
    & ffmpeg -y -i $a -i $b -filter_complex $fltGif -map "[gifv]" -loop 0 -an $OutGif
    Write-Host "[+] Exported $OutWebp, $OutGif, $OutMp4" -ForegroundColor Green
    exit 0
}

Write-Host "[*] Triggering synchronized Dual-PC capture via $ApiUrl/api/record/start ..." -ForegroundColor Cyan
$body = @{ slot = $Layout; auto_export = $true } | ConvertTo-Json
$startResp = Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/record/start" -ContentType "application/json" -Body $body
Write-Host ($startResp | ConvertTo-Json -Depth 4)

if ($Duration -gt 0) {
    Write-Host "[*] Recording both computers for $Duration seconds..." -ForegroundColor Yellow
    Start-Sleep -Seconds $Duration
} else {
    Read-Host "[*] Recording BOTH computers live! Press [Enter] to stop & stitch"
}

Write-Host "[*] Stopping captures & stitching..." -ForegroundColor Cyan
$null = Invoke-RestMethod -Method Post -Uri "$ApiUrl/api/record/stop"
for ($i = 0; $i -lt 60; $i++) {
    $st = Invoke-RestMethod -Method Get -Uri "$ApiUrl/api/record/status"
    if ($st.status -eq "ready" -or $st.status -eq "error") {
        $st | ConvertTo-Json -Depth 4
        break
    }
    Start-Sleep -Seconds 1
}
