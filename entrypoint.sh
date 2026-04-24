#!/bin/sh  
set -e  
  
# ── Server config ─────────────────────────────────────────────────────────────  
cat > /config.toml <<EOF  
[server]  
host = "0.0.0.0"  
port = 7860  
EOF  
  
# Optional: protect the API with a token  
# Set HF Secret: API_TOKEN=sk-something  
if [ -n "$API_TOKEN" ]; then  
    cat >> /config.toml <<EOF  
  
[[server.api_tokens]]  
token = "${API_TOKEN}"  
description = "hf-space"  
EOF  
fi  
  
# ── Accounts ──────────────────────────────────────────────────────────────────  
# Add accounts via numbered HF Secrets:  
#   DS_EMAIL_1, DS_PASSWORD_1  
#   DS_EMAIL_2, DS_PASSWORD_2  ... and so on  
#  
# For phone login instead of email:  
#   DS_MOBILE_1, DS_AREA_CODE_1, DS_PASSWORD_1  
  
i=1  
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
    i=$((i + 1))  
done  
  
# ── Start ─────────────────────────────────────────────────────────────────────  
exec /usr/local/bin/ds-free-api -c /config.toml
