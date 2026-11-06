#!/usr/bin/env bash
set -e

echo "=== Omarchy / Hyprland KVM Receiver Setup ==="

# 1. Install required packages
echo "[*] Installing pacman dependencies..."
sudo pacman -S --needed --noconfirm python python-pip python-evdev wl-clipboard libnotify

# 2. Add active user to the input group
echo "[*] Adding user $USER to input group..."
sudo usermod -aG input "$USER"

# 3. Create udev rule granting input group write access to uinput
echo "[*] Installing udev rule for /dev/uinput..."
sudo bash -c 'cat <<EOF > /etc/udev/rules.d/99-uinput.rules
KERNEL=="uinput", GROUP="input", MODE="0660"
EOF'

# 4. Load uinput module and reload udev subsystem
echo "[*] Loading uinput module..."
sudo modprobe uinput
sudo udevadm control --reload-rules && sudo udevadm trigger

# 5. Setup systemd user service
echo "[*] Setting up systemd user service..."
mkdir -p "$HOME/.config/systemd/user"
cat <<EOF > "$HOME/.config/systemd/user/kvm-client.service"
[Unit]
Description=Tailscale KVM Receiver Daemon
After=network.target

[Service]
ExecStart=/usr/bin/python3 %h/kvm_client.py
Restart=always
RestartSec=3

[Install]
WantedBy=default.target
EOF

systemctl --user daemon-reload
echo "[+] Setup completed!"
echo "    Note: If you were just added to the 'input' group, log out and back in (or run 'newgrp input') before starting the service."
echo "    To enable and start immediately: systemctl --user enable --now kvm-client.service"
