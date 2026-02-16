# LSSA Treasury Workshop

> **Build your first LEZ program — from zero to on-chain treasury in 60 minutes.**

This hands-on workshop walks you through building, deploying, and interacting with a treasury program on the Logos Execution Zone (LEZ). You'll learn PDAs, chained calls, and the full dev workflow.

## Prerequisites

- Rust nightly (edition 2024)
- [Risc0 toolchain](https://dev.risczero.com/api/zkvm/install): `curl -L https://risczero.com/install | bash && rzup install`
- A running LSSA sequencer (instructions below)
- ~30 min for build time on first compile (subsequent builds are fast)

## Workshop Outline

| # | Section | Time | What you'll learn |
|---|---------|------|-------------------|
| 1 | [Setup](#1-setup) | 10 min | Environment, sequencer, wallet |
| 2 | [Explore the Code](#2-explore-the-code) | 10 min | Project structure, core types |
| 3 | [Build & Deploy](#3-build--deploy) | 10 min | Compile zkVM guest, deploy programs |
| 4 | [Create a Vault](#4-create-a-vault) | 10 min | PDAs, account claiming, chained calls |
| 5 | [Send & Deposit](#5-send--deposit) | 10 min | PDA authorization, cross-program calls |
| 6 | [Challenge: Extend It](#6-challenge-extend-it) | 10 min | Add your own instruction |

---

## 1. Setup

### Clone and check the toolchain

```bash
git clone https://github.com/jimmy-claw/lssa-treasury.git
cd lssa-treasury
git checkout bedrock-api

# Verify Rust
rustup show  # Should show nightly

# Verify risc0
cargo risczero --version
```

### Start the sequencer

In a separate terminal:

```bash
git clone https://github.com/logos-blockchain/lssa.git
cd lssa/sequencer_runner
RUST_LOG=info cargo run $(pwd)/configs/debug
```

> **Note:** First build takes a while. Grab a coffee ☕

### Set up your wallet

```bash
cd lssa
cargo install --path wallet --force

# Create your identity
wallet account new public
# Save the output — this is your account ID
```

### Verify everything works

```bash
# Back in lssa-treasury
cargo check -p treasury_core
```

If this compiles, you're ready!

---

## 2. Explore the Code

### Project structure

```
lssa-treasury/
├── treasury_core/          ← Shared types (on-chain + off-chain)
│   └── src/lib.rs          ← Instructions, state, PDA helpers
├── treasury_program/       ← On-chain logic (runs in zkVM)
│   └── src/
│       ├── lib.rs
│       ├── create_vault.rs ← CreateVault handler
│       ├── send.rs         ← Send handler
│       └── receive.rs      ← Deposit handler
├── methods/                ← Risc0 build infrastructure
│   └── guest/src/bin/      ← zkVM entry point
└── examples/               ← Off-chain scripts
    └── program_deployment/
        └── src/bin/
            ├── deploy_and_create_vault.rs
            └── send_from_vault.rs
```

### 🔍 Exercise: Read the instruction enum

Open `treasury_core/src/lib.rs` and find the `Instruction` enum:

```rust
pub enum Instruction {
    CreateVault { token_name, initial_supply, token_program_id, authorized_accounts },
    Send { amount, token_program_id },
    Deposit { amount },
}
```

**Questions to consider:**
- Why does `CreateVault` take a `token_program_id`?
- Why does `Send` need `authorized_accounts` but `Deposit` doesn't?

### 🔍 Exercise: Understand PDAs

Find the PDA derivation functions:

```rust
pub fn compute_treasury_state_pda(program_id: &ProgramId) -> AccountId
pub fn compute_vault_holding_pda(program_id: &ProgramId, token_def: &AccountId) -> AccountId
```

**Key insight:** These are deterministic. Given the same inputs, anyone gets the same account ID. No private key exists for these accounts — only the program can authorize them.

```
PDA = hash("/NSSA/v0.2/AccountId/PDA/" || program_id || seed)
```

---

## 3. Build & Deploy

### Build the treasury program

```bash
# Build for zkVM (compiles to RISC-V)
cargo risczero build --manifest-path methods/guest/Cargo.toml

# The binary lands here:
ls target/riscv32im-risc0-zkvm-elf/docker/treasury.bin
```

### Build the token program

You also need the token program (from the LSSA repo):

```bash
cd /path/to/lssa
cargo risczero build --manifest-path programs/token/methods/guest/Cargo.toml
```

### Deploy both programs

```bash
export PROGRAMS_DIR=$(pwd)/target/riscv32im-risc0-zkvm-elf/docker

# Deploy treasury
wallet deploy-program $PROGRAMS_DIR/treasury.bin

# Deploy token
wallet deploy-program $PROGRAMS_DIR/token.bin
```

Each program gets a deterministic ID based on its bytecode hash.

---

## 4. Create a Vault

This is where it gets interesting! `CreateVault`:
1. Claims the treasury state PDA
2. Creates a new token definition
3. Mints tokens into a vault PDA
4. All in one atomic transaction via chained calls

### Run it

```bash
# Create an account for the token definition
wallet account new public
# → Public/<TOKEN_DEF_ID>

cd examples/program_deployment

cargo run --bin deploy_and_create_vault -- \
    $PROGRAMS_DIR/treasury.bin \
    $PROGRAMS_DIR/token.bin \
    <TOKEN_DEF_ID> \
    <YOUR_ACCOUNT_ID>   # authorized signer
```

### 🔍 Exercise: Trace the chained call

Open `treasury_program/src/create_vault.rs` and find:

```rust
let chained_call = ChainedCall::new(
    token_program_id,
    vec![token_definition.clone(), vault_for_chain],
    &token_core::Instruction::NewFungibleDefinition { ... },
)
.with_pda_seeds(vec![vault_holding_pda_seed(&token_definition.account_id)]);
```

**What's happening:**
1. Treasury builds a `ChainedCall` to the Token program
2. It passes the vault PDA with `is_authorized = true`
3. It provides the PDA seed so the runtime can verify the derivation
4. The Token program mints tokens into the vault — but only because Treasury authorized it

**Draw the flow:**
```
Treasury Program          Runtime              Token Program
     │                       │                       │
     ├── ChainedCall ───────►│                       │
     │   + PDA seed          ├── verify PDA ────────►│
     │   + authorized vault  │   hash matches? ✓     │
     │                       │                       │
     │                       ├── execute ────────────►│
     │                       │   NewFungibleDef       │
     │                       │   + mint to vault      │
```

---

## 5. Send & Deposit

### Send tokens from the vault

```bash
# Create a recipient
wallet account new public
# → Public/<RECIPIENT_ID>

cargo run --bin send_from_vault -- \
    $PROGRAMS_DIR/treasury.bin \
    $PROGRAMS_DIR/token.bin \
    <TOKEN_DEF_ID> \
    <RECIPIENT_ID> \
    100 \
    <YOUR_ACCOUNT_ID>   # must be an authorized signer
```

### 🔍 Exercise: Compare Send vs Deposit

| | Send | Deposit |
|---|------|---------|
| **Who authorizes?** | Treasury program (PDA) | User's signature |
| **PDA seeds needed?** | Yes (vault is sender) | No (vault is receiver) |
| **Authorization check** | `is_authorized = true` on vault | Transaction witness |

Open both `send.rs` and `receive.rs` — notice that `receive.rs` has no `.with_pda_seeds()`. Why?

> **Answer:** Deposits don't spend *from* the PDA vault, they spend *to* it. The sender is authorized by the user's transaction signature, not by PDA authority.

---

## 6. Challenge: Extend It

Now it's your turn! Pick one:

### Challenge A: Add a `Balance` instruction

Create an instruction that reads the vault balance without modifying state.

**Hints:**
- Add `Balance` to the `Instruction` enum in `treasury_core`
- Read `vault_holding.account.data` and decode it as `TokenHolding`
- Return the same `pre_states` as `post_states` (no changes)
- Use `env::commit()` in the guest to output the balance

### Challenge B: Add multiple authorized signers

The current code supports multiple signers via `authorized_accounts`. Try:
1. Deploy with 2 signers: `cargo run --bin deploy_and_create_vault -- ... <signer1> <signer2>`
2. Send using signer 2 instead of signer 1
3. Try sending with an unauthorized key — what happens?

### Challenge C: Build a multisig

Want a real challenge? Check out the `multisig` branch:

```bash
git checkout multisig
```

This extends the treasury with M-of-N threshold signatures. Study how:
- `MultisigState` tracks members and threshold
- `CreateMultisig` initializes the signer set
- `Execute` requires threshold witnesses
- The CLI provides `create`, `execute`, `add-member`, `set-threshold`

---

## Key Concepts Reference

| Concept | What | Why |
|---------|------|-----|
| **PDA** | Account derived from program ID + seed | Program-controlled accounts without private keys |
| **Chained Call** | One program calling another | Composability (Treasury → Token) |
| **PDA Seed** | 32-byte value for derivation | Proves PDA ownership to the runtime |
| **Account Claiming** | `AccountPostState::new_claimed()` | First-time PDA initialization |
| **Authorization** | `is_authorized = true` | Grants spending permission to chained programs |
| **Witness Set** | Signatures on the transaction | Proves user consent for account nonce bumps |

## Troubleshooting

| Problem | Solution |
|---------|----------|
| `risc0` version mismatch | Pin to `=3.0.4` in Cargo.toml |
| "account not found" | Make sure you created the account with `wallet account new public` |
| "unauthorized" on Send | Verify your signer ID matches one in `authorized_accounts` |
| Build takes forever | First build downloads/compiles everything — subsequent builds are fast |
| Sequencer not responding | Check it's running: `curl http://localhost:8080/health` |

## Next Steps

- Read the [LSSA README](https://github.com/logos-blockchain/lssa) for the full framework overview
- Study `programs/amm/` in LSSA for advanced PDA patterns (liquidity pools, swaps)
- Try the `multisig` branch for M-of-N governance
- Join the [Logos Discord](https://discord.gg/logos-state) to ask questions

---

*Built with 🦞 by Jimmy & Václav at Logos*
