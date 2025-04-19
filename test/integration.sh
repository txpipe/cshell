#! /bin/bash

CSHELL_STORE_PATH="$(dirname "$(cargo locate-project | jq -r '.root')")/test/integration.toml"
CSHELL_OUTPUT_FORMAT=json
CSHELL_LOG=error

run() {
  cargo run -q -- \
    -s "$CSHELL_STORE_PATH" \
    -o "$CSHELL_OUTPUT_FORMAT" \
    --log-level "$CSHELL_LOG" \
    "$@"
}

log() {
  local log_level="$1"
  local log_value="$2"

  echo "$(date +"%Y-%m-%d %H:%M:%S"): [$log_level] $log_value"
}

# Create wallet
OUTPUT=$(run wallet create --name test --password test)
NAME=$(echo "$OUTPUT" | jq -r '.name' )
MNEMONIC=$(echo "$OUTPUT" | jq -r '.mnemonic' )
PK=$(echo "$OUTPUT" | jq -r '.public_key' )
if [[ "$NAME" == "test" ]]; then
  log INFO "Succesfully created wallet"
else
  log ERROR "Failed to create wallet"
  log DEBUG "name: $NAME"
  log DEBUG "Output"
  log DEBUG "$OUTPUT"
  exit 1
fi

# Restore from mnemonic
OUTPUT=$(run wallet restore --name restored --password test --mnemonic "$MNEMONIC")
NAME=$(echo "$OUTPUT" | jq -r '.name' )
NEW_PK=$(echo "$OUTPUT" | jq -r '.public_key' )
if [[ "$PK" == "$NEW_PK" ]]; then
  log INFO "Succesfully restored wallet from mnemonic"
else
  log ERROR "Failed to restore wallet from mnemonic"
  log DEBUG "$OUTPUT"
  exit 1
fi

# Create provider
OUTPUT=$(run provider create \
  --name test \
  --is-default true \
  --network-kind testnet \
  --utxorpc-url "https://preprod.utxorpc-v0.demeter.run" \
  --utxorpc-headers '{"dmtr-api-key":"utxorpc1yzpcjdkeuqgy5pfg69z"}' \
  --trp-url "https://preprod.trp-m1.demeter.run" \
  --trp-headers '{"dmtr-api-key":"trpyzpcjdkeuqgy5pfg69z"}'
)
NAME=$(echo "$OUTPUT" | jq -r '.name' )
if [[ "$NAME" == "test" ]]; then
  log INFO "Succesfully created provider"
else
  log ERROR "Failed to create provider"
  log DEBUG "name: $NAME"
  log DEBUG "Output"
  log DEBUG "$OUTPUT"
  exit 1
fi

# Test provider
OUTPUT=$(run provider test)
if [[ "$OUTPUT" == *error* ]]; then
  log ERROR "Failed to connect to provider"
  log DEBUG "$OUTPUT"
  exit 1
else
  log INFO "Succesfully connected to provider"
fi

# Get balance
OUTPUT=$(run wallet balance)
COIN=$(echo "$OUTPUT" | jq -r '.coin')
if [[ "$COIN" == "0" ]]; then
  log INFO "Succesfully got balance"
else
  log ERROR "Failed to query balance"
  log DEBUG "$OUTPUT"
  exit 1
fi

# Edit wallet
OUTPUT=$(run wallet edit restored --new-name edited --is-default true)
IS_DEFAULT=$(echo "$OUTPUT" | jq -r '.is_default' )
if [[ "$IS_DEFAULT" == "true" ]]; then
  log INFO "Succesfully set restored wallet as default"
else
  log ERROR "Failed to edit default wallet"
  log DEBUG "$OUTPUT"
  exit 1
fi

# Delete wallets
run wallet delete test
run wallet delete edited
OUTPUT=$(run wallet list)
AMOUNT_OF_WALLETS=$(echo "$OUTPUT" | jq -r '. | length' )
if [[ "$AMOUNT_OF_WALLETS" == "0" ]]; then
  log INFO "Succesfully deleted all wallets"
else
  log ERROR "Failed to delete wallets"
  log DEBUG "$OUTPUT"
  exit 1
fi

# Delete provider
run provider delete test
OUTPUT=$(run provider list)
AMOUNT_OF_PROVIDERS=$(echo "$OUTPUT" | jq -r '. | length' )
if [[ "$AMOUNT_OF_PROVIDERS" == "0" ]]; then
  log INFO "Succesfully deleted all providers"
else
  log ERROR "Failed to delete providers"
  log DEBUG "$OUTPUT"
  exit 1
fi

# Cleanup
rm "$CSHELL_STORE_PATH"
