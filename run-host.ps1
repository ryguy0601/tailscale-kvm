# Universal Host Launcher for Windows
$repoDir = Split-Path -Parent $MyInvocation.MyCommand.Path
& "$repoDir\target\release\tailscale-kvm-host.exe" $args
