#!/bin/bash
set -e

# Script to install nettest systemd service file
# Usage: sudo ./install-systemd.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERVICE_FILE="nettest.service"
SYSTEMD_DIR="/usr/lib/systemd/system"
SERVICE_PATH="$SYSTEMD_DIR/$SERVICE_FILE"

# Check if running as root
if [ "$EUID" -ne 0 ]; then 
    echo "Error: This script must be run as root (use sudo)"
    exit 1
fi

# Check if service file exists
if [ ! -f "$SCRIPT_DIR/$SERVICE_FILE" ]; then
    echo "Error: Service file '$SERVICE_FILE' not found in $SCRIPT_DIR"
    exit 1
fi

# Check if systemd is available
if ! command -v systemctl &> /dev/null; then
    echo "Error: systemd is not available on this system"
    exit 1
fi

echo "Installing nettest systemd service..."

# Create systemd directory if it doesn't exist
mkdir -p "$SYSTEMD_DIR"

# Copy service file
echo "Copying $SERVICE_FILE to $SERVICE_PATH..."
cp "$SCRIPT_DIR/$SERVICE_FILE" "$SERVICE_PATH"

# Set proper permissions
chmod 644 "$SERVICE_PATH"

# Reload systemd daemon
echo "Reloading systemd daemon..."
systemctl daemon-reload

echo "Systemd service file installed successfully!"
echo ""
echo "To manage the service, use:"
echo "  systemctl enable nettest    # Enable service to start on boot"
echo "  systemctl start nettest     # Start the service"
echo "  systemctl stop nettest      # Stop the service"
echo "  systemctl restart nettest   # Restart the service"
echo "  systemctl status nettest    # Check service status"
echo "  systemctl disable nettest   # Disable service from starting on boot"

