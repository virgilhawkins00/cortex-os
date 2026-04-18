#!/bin/bash
# scripts/generate_certs.sh — Generate local CA and certificates for NATS mTLS.

set -e

CERT_DIR="./certs"
mkdir -p "$CERT_DIR"

echo "🔐 Generating CA..."
openssl genrsa -out "$CERT_DIR/ca-key.pem" 2048
openssl req -x509 -new -nodes -key "$CERT_DIR/ca-key.pem" -days 3650 -out "$CERT_DIR/ca.pem" -subj "/CN=Cortex-CA"

echo "🔐 Generating NATS Server Cert..."
openssl genrsa -out "$CERT_DIR/server-key.pem" 2048
openssl req -new -key "$CERT_DIR/server-key.pem" -out "$CERT_DIR/server.csr" -subj "/CN=localhost"
openssl x509 -req -in "$CERT_DIR/server.csr" -CA "$CERT_DIR/ca.pem" -CAkey "$CERT_DIR/ca-key.pem" -CAcreateserial -out "$CERT_DIR/server.pem" -days 365

echo "🔐 Generating Client Cert..."
openssl genrsa -out "$CERT_DIR/client-key.pem" 2048
openssl req -new -key "$CERT_DIR/client-key.pem" -out "$CERT_DIR/client.csr" -subj "/CN=cortex-client"
openssl x509 -req -in "$CERT_DIR/client.csr" -CA "$CERT_DIR/ca.pem" -CAkey "$CERT_DIR/ca-key.pem" -CAcreateserial -out "$CERT_DIR/client.pem" -days 365

echo "✅ Certificates generated in $CERT_DIR"
echo ""
echo "To use mTLS, update your NATS server config and set these variables:"
echo "NATS_CA_PATH=$CERT_DIR/ca.pem"
echo "NATS_CERT_PATH=$CERT_DIR/client.pem"
echo "NATS_KEY_PATH=$CERT_DIR/client-key.pem"
