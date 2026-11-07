# Universal Receiver Launcher for Windows
$repoDir = Split-Path -Parent $MyInvocation.MyCommand.Path
& "$repoDir\target\release\tailscale-kvm-receiver.exe" $args
