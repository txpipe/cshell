# C-Shell

A Cardano wallet built for developers and power users.

# Installation

## From Source

You'll need to have the following components already available in your system.

- [Rust toolchain](https://www.rust-lang.org/learn/get-started)

The following instructions show how to build and install _CShell_ from source code.

Use `git` to clone CShell source-code from our Github repository:

```sh
git clone https://github.com/txpipe/cshell.git
```

Use `cargo` to compile and install the generated binary for your user profile:

```sh
cargo install --all-features --path .
```

Once you completed the above steps, you should be able to call CShell directly from the command line:

```sh
cshell --help
```

## Binary Releases 

(Coming soon)

_CShell_ can be run as a standalone executable. The [Github
release](https://github.com/txpipe/cshell/releases/latest/) page includes the
binaries for different OS and architectures. It's a self-contained, single-file
binary that can be downloaded directly.

For simplicity, we also provide diferent installers for supported platform to
automate the installation process. Regardless of the installer, the outcome
should be the same, choose the one that fits your needs.

### Install via shell script

You can run the following command line script to install CShell on supported systems (Mac / Linux)

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/txpipe/cshell/releases/latest/download/cshell-installer.sh | sh
```

### Install via powershell script

You can use Powershell to install CShell on Windows systems.

```sh
powershell -c "irm https://github.com/txpipe/cshell/releases/latest/download/cshell-installer.ps1 | iex"
```

### Install via Homebrew

You can use Homebrew to install the latest version of CShell in supported
systems (Mac / Linux)

```sh
brew install txpipe/tap/cshell
```

## Download Binaries

|  File  | Platform |
|--------|----------|
| [cshell-aarch64-apple-darwin.tar.xz](https://github.com/txpipe/cshell/releases/latest/download/cshell-aarch64-apple-darwin.tar.xz) | Apple Silicon macOS |
| [cshell-x86_64-apple-darwin.tar.xz](https://github.com/txpipe/cshell/releases/latest/download/cshell-x86_64-apple-darwin.tar.xz) | Intel macOS |
| [cshell-x86_64-pc-windows-msvc.zip](https://github.com/txpipe/cshell/releases/latest/download/cshell-x86_64-pc-windows-msvc.zip) | x64 Windows |
| [cshell-x86_64-unknown-linux-gnu.tar.xz](https://github.com/txpipe/cshell/releases/latest/download/cshell-x86_64-unknown-linux-gnu.tar.xz) | x64 Linux |
| [cshell-aarch64-unknown-linux-gnu.tar.xz](https://github.com/txpipe/cshell/releases/latest/download/cshell-aarch64-unknown-linux-gnu.tar.xz) | ARM64 Linux |


# Usage

To run CShell you need, at least, 1 provider and 1 wallet.

To add a wallet, you can do the following:


```sh
cargo run -- wallet create

```

This will prompt you for a a name and a password. Keep in mind that losing the
password means loosing the private key, because it is encrypted.

To add a provider, you can do something similar:

```sh
cargo run -- provider create
```

This will prompt you for a name, a kind (only UTxORPC supported), whether it is for mainnet or testnet, a URL and the possibility to add headers.

>>> If you have a [Demeter](https://demeter.run) port you would have to set the URL as `https://{host}` and on put `dmtr-api-key:YOUR_API_KEY` on the headers.




