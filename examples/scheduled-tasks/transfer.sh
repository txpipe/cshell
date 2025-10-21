#!/bin/bash

alias cshell="cargo run -- "

# Validate arguments
if [ $# -ne 4 ]; then
  echo "Error: exactly 4 arguments are required"
  echo "Usage: $0 <sender_wallet> <receiver_wallet> <lovelace_amount> <cron_string>"
  echo "Example: $0 alice bob 1000000 '0 */2 * * *'"
  echo "Cron format: 'minute hour day month weekday'"
  exit 1
fi

# Get arguments
SENDER_WALLET=$1
RECEIVER_WALLET=$2
AMOUNT=$3
CRON_STRING=$4

# Validate sender wallet name is not empty
if [ -z "$SENDER_WALLET" ]; then
  echo "Error: sender wallet name cannot be empty"
  exit 1
fi

# Validate receiver wallet name is not empty
if [ -z "$RECEIVER_WALLET" ]; then
  echo "Error: receiver wallet name cannot be empty"
  exit 1
fi

# Validate amount is a positive integer
if ! [[ "$AMOUNT" =~ ^[0-9]+$ ]]; then
  echo "Error: amount must be a positive integer"
  exit 1
fi

# Validate cron string is not empty
if [ -z "$CRON_STRING" ]; then
  echo "Error: cron string cannot be empty"
  exit 1
fi

# Basic validation of cron string format (5 fields separated by spaces)
CRON_FIELDS=$(echo "$CRON_STRING" | awk '{print NF}')
if [ "$CRON_FIELDS" -ne 5 ]; then
  echo "Error: invalid cron string format. Must have 5 fields: 'minute hour day month weekday'"
  echo "Example: '0 */2 * * *' (every 2 hours)"
  exit 1
fi

# Get sender wallet info
SENDER_INFO=$(cshell wallet info --name $SENDER_WALLET 2>&1)
if [ $? -ne 0 ]; then
  echo "Error: sender wallet '$SENDER_WALLET' does not exist"
  exit 1
fi

SENDER_ADDRESS=$(echo "$SENDER_INFO" | grep "Address (testnet)" | awk '{print $5}')

# Get receiver wallet info
RECEIVER_INFO=$(cshell wallet info --name $RECEIVER_WALLET 2>&1)
if [ $? -ne 0 ]; then
  echo "Error: receiver wallet '$RECEIVER_WALLET' does not exist"
  exit 1
fi

RECEIVER_ADDRESS=$(echo "$RECEIVER_INFO" | grep "Address (testnet)" | awk '{print $5}')

echo "Scheduling transfer of $AMOUNT lovelaces from $SENDER_WALLET to $RECEIVER_WALLET, cron schedule: $CRON_STRING"

CSHELL_PATH=$(which cshell)
TX3_PATH=$(pwd)/transfer.tx3
UNBUFFER_PATH=$(which unbuffer)
CMD="$CSHELL_PATH tx invoke --tx3-file $TX3_PATH --signers $SENDER_WALLET --unsafe --tx3-args-json '{\"sender\":\"$SENDER_ADDRESS\",\"receiver\":\"$RECEIVER_ADDRESS\",\"quantity\":$AMOUNT}'"

crontab -l | { cat; echo "$CRON_STRING $UNBUFFER_PATH $CMD"; } | crontab -