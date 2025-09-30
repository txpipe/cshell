# TX3 Transaction Examples

This directory contains example TX3 files to execute through CShell invoke.

## Prerequisites

Before running TX3 files, ensure you have:

1. **CShell installed** - See the main [README](../README.md) for installation instructions
2. **A CShell configuration file** - Created using `cshell provider create` and `cshell wallet create`
3. **At least one provider configured** - For blockchain connectivity
4. **At least one wallet configured** - For signing transactions

## How to Execute TX3 Files

Use the following command pattern to execute a TX3 file:

```bash
cshell -s ./cshell.toml tx invoke --tx3-file ./<tx3-file>
```

### Command Breakdown

- `cshell` - Runs CShell
- `-s <config_file>` - Specifies the CShell configuration file path
- `tx invoke` - Invokes a transaction
- `--tx3-file <file_path>` - Path to the TX3 file to execute

### Configuration File

The `-s` flag points to your CShell configuration file (usually `cshell.toml`), which contains:
- Provider settings
- Wallet configurations

## Available Examples

### 1. `transfer.tx3` - Basic ADA Transfer

Transfers ADA from one party to another with change calculation.

**Parties**: `Sender`, `Receiver`  
**Parameters**: `quantity` (amount to transfer in lovelace)

```bash
cshell -s ~/.tx3/tmp/devnet_756435378c8d3771/cshell.toml tx invoke --tx3-file ./transfer.tx3
```

### 2. `mint_token.tx3` - Token Minting

Creates new native tokens on Cardano.

**Parties**: `Minter`  
**Parameters**: 
- `token_policy` (policy ID in bytes)
- `token_name` (token name in bytes)
- `quantity` (amount to mint)

```bash
cshell -s ~/.tx3/tmp/devnet_756435378c8d3771/cshell.toml tx invoke --tx3-file ./mint_token.tx3
```

### 3. `mint_with_script.tx3` - Plutus Script Token Minting

Demonstrates token minting using a Plutus v3 script with a PIN-based vending machine mechanism. This example shows how to interact with smart contracts for token minting.

**Parties**: `Customer`  
**Parameters**: 
- `pin` (PIN code in bytes)

```bash
cshell -s ~/.tx3/tmp/devnet_756435378c8d3771/cshell.toml tx invoke --tx3-file ./mint_with_script.tx3
```
