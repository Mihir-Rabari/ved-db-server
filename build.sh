#!/bin/bash
# Build script for VedDB Server and Core

echo "Building VedDB Server and Core..."
cargo build --release
if [ $? -ne 0 ]; then
    echo "Failed to build server and core"
    exit 1
fi

echo ""
echo "Build complete!"
echo "Server binary: target/release/veddb-server"
echo ""
echo "Note: To build the client, go to the clients/rust-client directory and run 'cargo build --release'"
