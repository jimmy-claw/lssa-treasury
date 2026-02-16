# LSSA Multisig — M-of-N Threshold Treasury for LEZ

An M-of-N multisig program for the [Logos Execution Zone (LEZ)](https://github.com/logos-blockchain/lssa). Multiple signers must approve transfers before they execute — no single key can drain the treasury.

📄 **[FURPS Specification](docs/FURPS.md)** — functional requirements, usability, reliability, performance, security constraints.

## How It Works

- **Create** a multisig with N members and threshold M
- **Execute** a transfer — requires M valid signer signatures in the transaction
- **Manage** members and threshold (add/remove members, change threshold) — also requires M signatures
- State lives in a **PDA** (Program Derived Account) — only the multisig program controls it

## Project Structure

```
lssa-treasury/
├── treasury_core/           — shared types, instructions, PDA helpers
├── treasury_program/        — on-chain handlers
│   └── src/
│       ├── create_multisig.rs
│       ├── execute.rs
│       ├── add_member.rs
│       ├── remove_member.rs
│       └── change_threshold.rs
├── methods/                 — risc0 zkVM guest build
│   └── guest/src/bin/treasury.rs
├── examples/
│   └── program_deployment/
│       └── src/bin/treasury.rs  — unified CLI
└── docs/
    └── FURPS.md             — requirements specification
```

## Quick Start

### Prerequisites

- Rust nightly (edition 2024)
- [Risc0 toolchain](https://dev.risczero.com/api/zkvm/install): `curl -L https://risczero.com/install | bash && rzup install`
- A running LSSA sequencer

### Build

```bash
# Check core logic
cargo check -p treasury_core -p treasury_program

# Build the zkVM guest (produces the on-chain binary)
cargo risczero build --manifest-path methods/guest/Cargo.toml

# Build the CLI
cargo build --bin treasury -p program_deployment
```

### Deploy

```bash
# Start the sequencer (from lssa repo)
cd /path/to/lssa/sequencer_runner
RUST_LOG=info cargo run $(pwd)/configs/debug

# Deploy the multisig program
wallet deploy-program target/riscv32im-risc0-zkvm-elf/docker/treasury.bin
```

## CLI Usage

The `treasury` binary is a single unified CLI for all multisig operations.

```bash
# Set program binary path (or use -p flag each time)
export MULTISIG_PROGRAM=artifacts/multisig.bin
```

### Create a 2-of-3 multisig

```bash
treasury create --threshold 2 --members <ALICE_ID> <BOB_ID> <CAROL_ID>
```

### Check multisig status

```bash
treasury status
```

### Execute a transfer (needs M signers)

```bash
treasury execute --recipient <ACCOUNT_ID> --amount 1000 --signer <YOUR_ID>
```

Each signer submits the same `execute` command. Once M signatures are collected in the transaction, it goes through.

### Manage members

```bash
# Add a member (requires M current signatures)
treasury add-member --member <NEW_MEMBER_ID>

# Remove a member (requires M current signatures)
treasury remove-member --member <MEMBER_ID>

# Change threshold
treasury set-threshold --threshold 3
```

### Shell completions

```bash
# Generate completions for your shell
treasury completions --shell bash > /etc/bash_completion.d/treasury
treasury completions --shell zsh > ~/.zfunc/_treasury
treasury completions --shell fish > ~/.config/fish/completions/treasury.fish
```

## Tests

```bash
cargo test -p treasury_program
```

18 unit tests covering creation, execution, member management, threshold changes, and edge cases (duplicate members, threshold bounds, replay protection via nonce).

## References

- [LSSA Repository](https://github.com/logos-blockchain/lssa)
- [FURPS Specification](docs/FURPS.md)
- [Workshop Guide](WORKSHOP.md) *(on `bedrock-api` branch)*
