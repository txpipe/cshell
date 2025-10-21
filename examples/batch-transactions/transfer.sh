#!/bin/bash

# Validate arguments
if [ $# -ne 3 ]; then
  echo "Error: exactly 3 arguments are required"
  echo "Usage: $0 <sender_wallet> <receiver_wallets_list> <lovelace_amount>"
  echo "Example: $0 alice bob,charlie,mark 1000000"
  exit 1
fi

# Get arguments
SENDER_WALLET=$1
RECEIVER_WALLETS=$2
AMOUNT=$3

# Validate sender wallet name is not empty
if [ -z "$SENDER_WALLET" ]; then
  echo "Error: sender wallet name cannot be empty"
  exit 1
fi

# Validate receiver wallets list is not empty
if [ -z "$RECEIVER_WALLETS" ]; then
  echo "Error: receiver wallets list cannot be empty"
  exit 1
fi

# Validate amount is a positive integer
if ! [[ "$AMOUNT" =~ ^[0-9]+$ ]]; then
  echo "Error: amount must be a positive integer"
  exit 1
fi

# Get sender wallet info
SENDER_INFO=$(cshell wallet info --name $SENDER_WALLET 2>&1)
if [ $? -ne 0 ]; then
  echo "Error: sender wallet '$SENDER_WALLET' does not exist"
  exit 1
fi

SENDER_ADDRESS=$(echo "$SENDER_INFO" | grep "Address (testnet)" | awk '{print $5}')

for RECEIVER_WALLET in ${RECEIVER_WALLETS//,/ }; do

  echo "Sending $AMOUNT lovelaces from $SENDER_WALLET to $RECEIVER_WALLET"

  RECEIVER_INFO=$(cshell wallet info --name $RECEIVER_WALLET 2>&1)
  if [ $? -ne 0 ]; then
    echo "Error: receiver wallet '$RECEIVER_WALLET' does not exist"
    exit 1
  fi

  RECEIVER_ADDRESS=$(echo "$RECEIVER_INFO" | grep "Address (testnet)" | awk '{print $5}')

  cshell tx invoke \
    --tx3-file ./transfer.tx3 --signers $SENDER_WALLET --unsafe \
    --tx3-args-json "{\"sender\":\"$SENDER_ADDRESS\",\"receiver\":\"$RECEIVER_ADDRESS\",\"quantity\":$AMOUNT}"

  echo ""

done