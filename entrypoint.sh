#!/bin/sh
set -e

# Hugging Face Spaces run as a non-root user (UID 1000)
# We should write the config to a writable directory like /tmp or the current WORKDIR
CONFIG_FILE="/tmp/config.toml"

echo "[entrypoint] Starting ds-free-api entrypoint..."
echo "[entrypoint] Generating config at ${CONFIG_FILE}"

cat > "${CONFIG_FILE}" <<EOF
[server]
host = "0.0.0.0"
port = 7860
EOF

i=1
ACCOUNT_COUNT=0
while true; do
    eval "password=\${DS_PASSWORD_$i}"
    [ -z "$password" ] && break

    eval "email=\${DS_EMAIL_$i:-}"
    eval "mobile=\${DS_MOBILE_$i:-}"
    eval "area_code=\${DS_AREA_CODE_$i:-}"

    cat >> "${CONFIG_FILE}" <<EOF

[[accounts]]
email = "${email}"
mobile = "${mobile}"
area_code = "${area_code}"
password = "${password}"
EOF
    ACCOUNT_COUNT=$((ACCOUNT_COUNT + 1))
    i=$((i + 1))
done

echo "[entrypoint] Generated config with ${ACCOUNT_COUNT} account(s)"
echo "[entrypoint] Server: 0.0.0.0:7860"

if [ "$ACCOUNT_COUNT" -eq 0 ]; then
    echo "[entrypoint] ERROR: No accounts configured. Set DS_EMAIL_1 and DS_PASSWORD_1 secrets in HF Space settings."
    # List environment variables for debugging (excluding passwords)
    env | grep DS_ | grep -v PASSWORD || true
    exit 1
fi

# Enable debug logging from the binary
export RUST_LOG=info
# Ensure stdout/stderr are unbuffered
export PYTHONUNBUFFERED=1

echo "[entrypoint] Executing ds-free-api..."
exec /usr/local/bin/ds-free-api -c "${CONFIG_FILE}"

