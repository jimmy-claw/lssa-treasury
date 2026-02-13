# Treasury Program — PDA (Program Derived Accounts) Example

A demonstration program for the [Logos Execution Zone (LEZ)](https://github.com/logos-blockchain/lssa) that shows how programs can own and control accounts through **Program Derived Accounts (PDAs)**, and compose with other programs through **chained calls**.

## What Does This Program Do?

The Treasury program acts as an on-chain vault manager. It can:

1. **Create Vaults** — deploy a new token and mint initial supply into a treasury-controlled vault
2. **Send** — transfer tokens from a vault to any recipient
3. **Deposit** — receive tokens from external senders into a vault

All vault accounts are **PDAs** — accounts whose authority is derived from the Treasury program itself, not from any external key. This means only the Treasury program can authorize actions on its vaults.

## Understanding PDAs

### What is a PDA?

A **Program Derived Account (PDA)** is an account whose ID (address) is deterministically computed from:
- A **program ID** (which program is the "authority" of the PDA)
- A **seed** (a 32-byte value that makes each PDA unique)

```
PDA Account ID = hash("/NSSA/v0.2/AccountId/PDA/" || program_id || seed)
```

PDAs are special because:
- **No private key** corresponds to them — nobody can sign for them externally
- **Only the deriving program** can authorize operations on them (by providing the seed)
- **Deterministic** — anyone can recompute the address given the program ID and seed

### Authority vs Ownership

In NSSA/LEZ, there are two distinct concepts that control who can modify an account:

| Concept | Meaning | Who? |
|---------|---------|------|
| **Program Ownership** | Which program can mutate the account's `data` and `balance` fields | Set when a program "claims" the account |
| **Authority** | Who can set `is_authorized = true` on the account | For PDAs: the program that derived the account ID |

A typical pattern (used in this program):
1. The **Treasury program** derives a vault PDA and is its **authority**
2. The **Token program** claims the vault account and becomes its **owner** (it writes balance data)
3. When Treasury wants to spend from the vault, it sets `is_authorized = true` and provides the PDA seed
4. The Token program sees the authorized flag and executes the transfer

### PDA Derivation in This Program

```
┌─────────────────────────────────────────────────────┐
│                   Treasury Program                   │
│                  (treasury_program_id)               │
└──────────┬──────────────────────┬───────────────────┘
           │                      │
           │ seed: padded         │ seed: token_definition_id
           │ "treasury_state"     │       bytes ([u8; 32])
           │                      │
           ▼                      ▼
    ┌──────────────┐      ┌──────────────────┐
    │ Treasury     │      │ Vault Holding    │
    │ State PDA    │      │ PDA              │
    │              │      │                  │
    │ Owned by:    │      │ Owned by:        │
    │  Treasury    │      │  Token program   │
    │  program     │      │  (after claim)   │
    │              │      │                  │
    │ Authority:   │      │ Authority:       │
    │  Treasury    │      │  Treasury        │
    │  program     │      │  program         │
    └──────────────┘      └──────────────────┘
```

- **Treasury State PDA**: stores vault count — owned and controlled entirely by Treasury
- **Vault Holding PDA**: one per token — owned by Token program (holds balance data), but authorized by Treasury

## Project Structure

```
lssa-treasury/
├── Cargo.toml                    — workspace definition
├── README.md                     — this file
├── treasury_core/                — shared types (used on-chain and off-chain)
│   └── src/lib.rs                — Instruction enum, TreasuryState, PDA helpers
├── treasury_program/             — on-chain program logic
│   └── src/
│       ├── lib.rs
│       ├── create_vault.rs       — CreateVault handler
│       ├── send.rs               — Send handler
│       └── receive.rs            — Deposit handler
├── methods/                      — risc0 build infrastructure
│   ├── build.rs                  — embeds guest ELF via risc0_build
│   ├── src/lib.rs                — re-exports embedded methods
│   └── guest/
│       └── src/bin/treasury.rs   — zkVM guest binary entry point
└── examples/
    └── program_deployment/       — off-chain runner scripts
        └── src/bin/
            ├── deploy_and_create_vault.rs
            └── send_from_vault.rs
```

## Code Walkthrough

### 1. PDA Derivation (`treasury_core/src/lib.rs`)

The core crate provides deterministic PDA computation using `AccountId::from((&ProgramId, &PdaSeed))` — the same mechanism used by the NSSA runtime:

```rust
/// Fixed 32-byte seed for the treasury state PDA (padded with zeroes).
const TREASURY_STATE_SEED: [u8; 32] = { /* b"treasury_state" padded to 32 bytes */ };

/// Compute the treasury state PDA account ID.
pub fn compute_treasury_state_pda(treasury_program_id: &ProgramId) -> AccountId {
    AccountId::from((treasury_program_id, &treasury_state_pda_seed()))
}

/// Compute the vault holding PDA for a given token definition.
/// Uses the token definition's AccountId bytes as the seed.
pub fn compute_vault_holding_pda(
    treasury_program_id: &ProgramId,
    token_definition_id: &AccountId,
) -> AccountId {
    AccountId::from((treasury_program_id, &vault_holding_pda_seed(token_definition_id)))
}
```

The `PdaSeed` constructors wrap 32-byte arrays:

```rust
pub fn treasury_state_pda_seed() -> PdaSeed {
    PdaSeed::new(TREASURY_STATE_SEED)
}

pub fn vault_holding_pda_seed(token_definition_id: &AccountId) -> PdaSeed {
    PdaSeed::new(*token_definition_id.value())
}
```

These functions are used both inside the zkVM (by the program) and off-chain (by deployment scripts) to derive the same addresses.

### 2. CreateVault (`treasury_program/src/create_vault.rs`)

This instruction demonstrates three key patterns:

**a) First-time PDA claiming:**
```rust
let treasury_post_state = if treasury_state.account == Account::default() {
    // First call — claim the PDA for this program
    AccountPostState::new_claimed(treasury_post)
} else {
    // Already claimed — just update
    AccountPostState::new(treasury_post)
};
```

**b) Authorizing a PDA in a chained call:**
```rust
// Mark the vault as authorized — Treasury is the authority of this PDA
let mut vault_for_chain = vault_holding.clone();
vault_for_chain.is_authorized = true;
```

**c) Building a chained call with PDA seeds:**
```rust
let chained_call = ChainedCall::new(
    token_program_id,
    vec![token_definition.clone(), vault_for_chain],
    &token_core::Instruction::NewFungibleDefinition {
        name: token_name,
        total_supply: initial_supply,
    },
)
// Provide the seed so the runtime can verify: hash(treasury_id, seed) == vault PDA
.with_pda_seeds(vec![vault_holding_pda_seed(&token_definition.account_id)]);
```

### 3. Send (`treasury_program/src/send.rs`)

Demonstrates transferring *from* a PDA vault. The key insight: the vault is owned by the Token program (which manages balances), but the Treasury program is its authority (it can authorize spending).

```rust
// Look up the token definition to compute the correct PDA seed
let vault_token_holding = token_core::TokenHolding::try_from(&vault_holding.account.data)
    .expect("Vault must be a valid TokenHolding");
let definition_id = vault_token_holding.definition_id();

// Authorize the vault PDA
let mut vault_for_chain = vault_holding.clone();
vault_for_chain.is_authorized = true;

// Chain to Token::Transfer with PDA proof
let chained_call = ChainedCall::new(
    token_program_id,
    vec![vault_for_chain, recipient_holding.clone()],
    &token_core::Instruction::Transfer { amount_to_transfer: amount },
)
.with_pda_seeds(vec![vault_holding_pda_seed(&definition_id)]);
```

### 4. Deposit (`treasury_program/src/receive.rs`)

Deposits are simpler — no PDA authorization needed because the vault is the *receiver*, not the sender:

```rust
// The sender is authorized by the user's signature in the transaction.
// We just chain to Token::Transfer: sender → vault
let chained_call = ChainedCall::new(
    token_program_id,
    vec![sender_holding.clone(), vault_holding.clone()],
    &token_core::Instruction::Transfer { amount_to_transfer: amount },
);
// No .with_pda_seeds() — only needed when spending FROM a PDA
```

### 5. Guest Binary (`methods/guest/src/bin/treasury.rs`)

The guest binary is the entry point compiled to RISC-V for the zkVM. It reads inputs, dispatches to the right handler, and writes outputs:

```rust
fn main() {
    let (ProgramInput { pre_states, instruction }, instruction_words)
        = read_nssa_inputs::<Instruction>();

    let pre_states_clone = pre_states.clone();

    let (post_states, chained_calls) = match instruction {
        Instruction::CreateVault { .. } => { /* dispatch to create_vault */ }
        Instruction::Send { .. } =>       { /* dispatch to send */ }
        Instruction::Deposit { .. } =>    { /* dispatch to deposit */ }
    };

    // Use the chained-call variant since all instructions may produce chained calls
    write_nssa_outputs_with_chained_call(
        instruction_words, pre_states_clone, post_states, chained_calls,
    );
}
```

## Build & Run

### Prerequisites

- Rust (edition 2024 / nightly)
- [Risc0 toolchain](https://dev.risczero.com/api/zkvm/install): `curl -L https://risczero.com/install | bash && rzup install`

### Check the core logic compiles

```bash
cargo check -p treasury_core -p treasury_program
```

### Build the guest binary (needs risc0 toolchain)

```bash
cargo risczero build --manifest-path methods/guest/Cargo.toml
```

The compiled ELF will be in `target/riscv32im-risc0-zkvm-elf/docker/treasury.bin`.

### Deploy and run (needs a running sequencer)

```bash
# 1. Start the sequencer (from the lssa repo)
cd /path/to/lssa/sequencer_runner
RUST_LOG=info cargo run $(pwd)/configs/debug

# 2. Install the wallet CLI (from the lssa repo root)
cargo install --path wallet --force

# 3. Deploy the treasury + token programs
export PROGRAMS_DIR=$(pwd)/target/riscv32im-risc0-zkvm-elf/docker
wallet deploy-program $PROGRAMS_DIR/treasury.bin
wallet deploy-program $PROGRAMS_DIR/token.bin    # from lssa repo build
```

### CreateVault — create a token + mint into treasury vault

This instruction needs **3 accounts** (all PDAs, computed deterministically):

| # | Account | What it is | How to get the ID |
|---|---------|-----------|-------------------|
| 0 | `treasury_state` | Treasury metadata PDA | `compute_treasury_state_pda(treasury_program_id)` |
| 1 | `token_definition` | New token definition (uninitialized) | `wallet account new public` |
| 2 | `vault_holding` | Vault for minted tokens (PDA) | `compute_vault_holding_pda(treasury_program_id, token_def_id)` |

```bash
# Create a public account for the token definition
wallet account new public
# Output: Generated new account with account_id Public/<TOKEN_DEF_ID>

# Run CreateVault (the runner computes PDA IDs internally)
cd examples/program_deployment
cargo run --bin deploy_and_create_vault \
    $PROGRAMS_DIR/treasury.bin \
    <TREASURY_STATE_PDA_ID> \
    <TOKEN_DEF_ID> \
    <VAULT_HOLDING_PDA_ID>
```

> **Note:** The treasury_state and vault_holding PDA IDs are deterministic — they can be computed
> off-chain using `treasury_core::compute_treasury_state_pda()` and
> `treasury_core::compute_vault_holding_pda()`. The example runner should ideally compute these
> automatically (TODO).

### Send — transfer tokens from vault to a recipient

This instruction needs **3 accounts:**

| # | Account | What it is |
|---|---------|-----------|
| 0 | `treasury_state` | Treasury metadata PDA (same as above) |
| 1 | `vault_holding` | Vault PDA holding tokens |
| 2 | `recipient_holding` | Recipient's token holding account |

```bash
# Create a recipient account
wallet account new public
# Output: Generated new account with account_id Public/<RECIPIENT_ID>

cargo run --bin send_from_vault \
    $PROGRAMS_DIR/treasury.bin \
    <TREASURY_STATE_PDA_ID> \
    <VAULT_HOLDING_PDA_ID> \
    <RECIPIENT_ID> \
    100   # amount to send
```

### Deposit — receive tokens into the vault from an external sender

This instruction needs **3 accounts:**

| # | Account | What it is |
|---|---------|-----------|
| 0 | `treasury_state` | Treasury metadata PDA |
| 1 | `sender_holding` | Sender's token holding (authorized by user signature) |
| 2 | `vault_holding` | Vault PDA receiving tokens |

## Chained Call Flow

Here's the full execution flow for a `Send` instruction:

```
User submits transaction
    │
    │  Accounts: [treasury_state, vault_holding, recipient_holding]
    │  Instruction: Send { amount: 100, token_program_id }
    │
    ▼
┌─────────────────────────────────────────────────────┐
│  1. NSSA Runtime executes Treasury program           │
│                                                      │
│     treasury_program::send::send()                   │
│       ├─ Read vault token holding data               │
│       ├─ Set vault_holding.is_authorized = true      │
│       ├─ Build ChainedCall to Token::Transfer        │
│       │   └─ .with_pda_seeds([vault_seed])           │
│       └─ Return post_states + chained_calls          │
│                                                      │
│  2. Runtime verifies PDA:                            │
│     hash(treasury_program_id, vault_seed)            │
│       == vault_holding.account_id  ✓                 │
│                                                      │
│  3. Runtime executes chained call: Token::Transfer   │
│       ├─ vault_holding (authorized) → sender         │
│       ├─ recipient_holding → receiver                │
│       └─ Debit vault, credit recipient               │
│                                                      │
│  4. All state changes committed atomically           │
└─────────────────────────────────────────────────────┘
```

## Key Patterns Summary

| Pattern | Code | Purpose |
|---------|------|---------|
| PDA derivation | `AccountId::from((&program_id, &PdaSeed::new(seed)))` | Deterministic address from program + seed |
| PDA authorization | `account.is_authorized = true` | Grant authority to chained program |
| PDA proof | `.with_pda_seeds(vec![seed])` | Prove to runtime you derived this PDA |
| Account claiming | `AccountPostState::new_claimed(account)` | First-time PDA ownership |
| Conditional claim | Check `account == Account::default()` | Claim only if uninitialized |
| Chained call | `ChainedCall::new(program_id, accounts, &instruction)` | Cross-program invocation |
| Output with chains | `write_nssa_outputs_with_chained_call(...)` | Return results + chained calls |

## References

- [LSSA Repository](https://github.com/logos-blockchain/lssa) — full framework source
- `programs/amm/` — AMM program (advanced PDA usage: pool + vault + liquidity token PDAs)
- `programs/token/` — Token program (the program we chain to)
- `nssa/core/src/program.rs` — core types (`ProgramInput`, `ChainedCall`, `PdaSeed`, etc.)
- `examples/program_deployment/README.md` — step-by-step deployment tutorial
