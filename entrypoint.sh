#!/bin/sh  
set -e  
  
cat > /config.toml <<EOF  
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
  
    cat >> /config.toml <<EOF  
  
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
    exit 1  
fi  
  
# Enable debug logging from the binary  
export RUST_LOG=info  
  
exec /usr/local/bin/ds-free-api -c /config.toml
