#!/bin/bash

alias cshell="cargo run -- "

# Validate arguments
if [ $# -ne 3 ]; then
  echo "Error: exactly 3 arguments are required"
  echo "Usage: $0 <player_wallet> <pos_x> <pos_y>"
  echo "Example: $0 alice 25 25"
  exit 1
fi

# Get arguments
PLAYER_WALLET=$1
POS_X=$2
POS_Y=$3

# Validate player wallet name is not empty
if [ -z "$PLAYER_WALLET" ]; then
  echo "Error: player wallet name cannot be empty"
  exit 1
fi

# Validate POS_X is an integer
if ! [[ "$POS_X" =~ ^-?[0-9]+$ ]]; then
  echo "Error: pos_x must be an integer"
  exit 1
fi

# Validate POS_Y is an integer
if ! [[ "$POS_Y" =~ ^-?[0-9]+$ ]]; then
  echo "Error: pos_y must be an integer"
  exit 1
fi

# Get player wallet info
PLAYER_INFO=$(cshell wallet info --name $PLAYER_WALLET 2>&1)
if [ $? -ne 0 ]; then
  echo "Error: player wallet '$PLAYER_WALLET' does not exist"
  exit 1
fi

PLAYER_ADDRESS=$(echo "$PLAYER_INFO" | grep "Address (testnet)" | awk '{print $5}')

NEXT_SHIP_RES=$(curl --location 'https://8000-skillful-employee-kb9ou6.us1.demeter.run/graphql' --header 'Content-Type: application/json' \
  --data '{"query":"query { nextShipTokenName(spacetimeAddress: \"addr_test1wzmvtc20xxhseyj3tns4vcj6l3r5nccvamch3nawr7ffllcmwmxeq\", spacetimePolicyId: \"b6c5e14f31af0c92515ce156625afc4749e30ceef178cfae1f929fff\") { shipName pilotName } }"}')

SHIP_NAME=$(echo $NEXT_SHIP_RES | jq -r '.data.nextShipTokenName.shipName')
PILOT_NAME=$(echo $NEXT_SHIP_RES | jq -r '.data.nextShipTokenName.pilotName')

SHIP_NAME=$(printf '%s' "$SHIP_NAME" | xxd -p -u)
PILOT_NAME=$(printf '%s' "$PILOT_NAME" | xxd -p -u)

LAST_SLOT_RES=$(curl --location 'https://8000-skillful-employee-kb9ou6.us1.demeter.run/graphql' --header 'Content-Type: application/json' \
  --data '{"query":"query { lastSlot { slot } }"}')

TIP_SLOT=`expr $(echo $LAST_SLOT_RES | jq -r '.data.lastSlot.slot') + 300`

LAST_MOVE_TIMESTAMP=`expr $(date +%s) + 300000`

JSON=$(jq -n \
  --arg player "$PLAYER_ADDRESS" \
  --arg ship_name "$SHIP_NAME" \
  --arg pilot_name "$PILOT_NAME" \
  --argjson p_pos_x "$POS_X" \
  --argjson p_pos_y "$POS_Y" \
  --argjson tip_slot "$TIP_SLOT" \
  --argjson last_move_timestamp "$LAST_MOVE_TIMESTAMP" \
  '{
    "player": $player,
    "ship_name": $ship_name,
    "pilot_name": $pilot_name,
    "p_pos_x": $p_pos_x,
    "p_pos_y": $p_pos_y,
    "tip_slot": $tip_slot,
    "last_move_timestamp": $last_move_timestamp
  }'
)

cshell tx invoke --tx3-file ./asteria.tx3 --signers $PLAYER_WALLET --tx3-args-json "$JSON" --unsafe