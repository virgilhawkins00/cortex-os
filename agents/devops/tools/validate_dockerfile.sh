#!/bin/bash
# Mock Dockerfile validator
FILE_PATH=$1

if [ -z "$FILE_PATH" ]; then
    echo "Usage: sc_validate_dockerfile <path_to_dockerfile>"
    exit 1
fi

echo "--- Validating Dockerfile at $FILE_PATH ---"
if grep -q "FROM" "$FILE_PATH"; then
    echo "Check: Base image found."
else
    echo "Error: Missing FROM instruction."
    exit 1
fi

echo "STATUS: VALID"
echo "--------------------------------"
