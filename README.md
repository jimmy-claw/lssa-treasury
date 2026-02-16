# LSSA Treasury — DX Framework PoC

This branch demonstrates the **nssa-framework** proc macros applied to a real LEZ program. The treasury handles token vaults with PDAs and chained calls — complex enough to validate the framework handles real patterns.

📄 **[FURPS Specification](docs/FURPS.md)** — requirements for the multisig extension (see `multisig` branch).

> **Other branches:** [`bedrock-api`](../../tree/bedrock-api) (1-of-N treasury + workshop), [`multisig`](../../tree/multisig) (M-of-N threshold + CLI)

## What the Framework Does

The `#[nssa_program]` macro eliminates the boilerplate every LEZ program needs:

| Before (manual) | After (framework) |
|---|---|
| Define `Instruction` enum by hand | Auto-generated from function signatures |
| Write `main()`: read → match → dispatch → write | Auto-generated |
| Manual `accounts[0]`, `accounts[1]` indexing | Named destructuring with count validation |
| `panic!` / `assert!` for errors | `Result<NssaOutput, NssaError>` with error codes |

### Before — guest binary (20+ lines of boilerplate)

```rust
fn main() {
    let (ProgramInput { pre_states, instruction }, instruction_words)
        = read_nssa_inputs::<Instruction>();
    let pre_states_clone = pre_states.clone();
    let (post_states, chained_calls) = treasury_program::process(&pre_states, &instruction);
    write_nssa_outputs_with_chained_call(instruction_words, pre_states_clone, post_states, chained_calls);
}
```

Plus a separate `Instruction` enum, a `process()` dispatcher, and manual account slicing in every handler.

### After — just business logic

```rust
#[nssa_program]
mod treasury_program {
    use super::*;

    #[instruction]
    pub fn create_vault(
        treasury_state_acct: AccountWithMetadata,  // accounts[0]
        token_definition: AccountWithMetadata,      // accounts[1]
        vault_holding: AccountWithMetadata,          // accounts[2]
        token_name: [u8; 6],                         // ← instruction args
        initial_supply: u128,                        //   (become enum fields)
        token_program_id: ProgramId,
        authorized_accounts: Vec<[u8; 32]>,
    ) -> NssaResult {
        // Pure business logic — no boilerplate
        // ...
        Ok(NssaOutput::with_chained_calls(post_states, vec![chained_call]))
    }

    #[instruction]
    pub fn send(/* ... */) -> NssaResult { /* ... */ }

    #[instruction]
    pub fn deposit(/* ... */) -> NssaResult { /* ... */ }
}
```

The macro generates:
1. `Instruction` enum: `CreateVault { token_name, initial_supply, ... }`, `Send { ... }`, `Deposit { ... }`
2. `main()` with `read_nssa_inputs` → match dispatch → `write_nssa_outputs_with_chained_call`
3. Account array destructuring with count validation

## Project Structure

```
lssa-treasury/
├── treasury_core/               — PDA helpers, TreasuryState type
├── treasury_program/            — legacy handlers (pre-framework, for reference)
├── methods/
│   └── guest/src/bin/
│       └── treasury.rs          — ★ framework version (3 #[instruction] fns)
├── nssa-framework/              — umbrella crate (re-exports)
├── nssa-framework-core/         — NssaOutput, NssaError, NssaResult types
├── nssa-framework-macros/       — #[nssa_program] proc macro
├── examples/
│   └── program_deployment/      — off-chain runners
└── docs/
    └── FURPS.md                 — requirements spec
```

## Framework Crates

| Crate | Purpose |
|---|---|
| `nssa-framework` | Umbrella — `use nssa_framework::prelude::*` |
| `nssa-framework-core` | `NssaOutput`, `NssaError`, `NssaResult` — wraps real `nssa_core` types |
| `nssa-framework-macros` | `#[nssa_program]`, `#[instruction]` proc macros |

### Error Handling

```rust
// Before: panic!
assert!(accounts.len() == 3, "CreateVault requires exactly 3 accounts");

// After: structured errors with codes
if authorized_accounts.is_empty() {
    return Err(NssaError::custom(1, "At least one authorized account required"));
}

// Built-in error variants:
Err(NssaError::Unauthorized { message: "Signer not authorized".into() })
Err(NssaError::DeserializationError { account_index: 0, message: e.to_string() })
Err(NssaError::AccountCountMismatch { expected: 3, actual: v.len() })
```

### NssaOutput

```rust
// States only (no cross-program calls)
Ok(NssaOutput::states_only(vec![post_state_1, post_state_2]))

// States + chained calls (CPI with PDA seeds)
Ok(NssaOutput::with_chained_calls(
    vec![treasury_post, token_def_post, vault_post],
    vec![chained_call],
))
```

## Quick Start

### Prerequisites

- Rust nightly (edition 2024)
- [Risc0 toolchain](https://dev.risczero.com/api/zkvm/install): `curl -L https://risczero.com/install | bash && rzup install`

### Build

```bash
# Check everything compiles (host target)
cargo check

# Build the zkVM guest
cargo risczero build --manifest-path methods/guest/Cargo.toml

# Build the tooling (IDL generator + CLI)
cargo build --bin generate_idl --bin treasury_cli -p treasury-examples
```

### Generate the IDL

The `#[nssa_program]` macro captures the full program interface. Generate the IDL JSON:

```bash
cargo run --bin generate_idl > treasury-idl.json
```

This produces a JSON file describing every instruction, its accounts (with PDA info), argument types, account types, and errors. Example snippet:

```json
{
  "name": "treasury_program",
  "instructions": [
    {
      "name": "send",
      "accounts": [
        { "name": "treasury_state_acct", "pda": { "seeds": [{"kind": "const", "value": "treasury_state"}] } },
        { "name": "vault_holding", "pda": { "seeds": [{"kind": "account", "path": "token_definition"}] } },
        { "name": "recipient" },
        { "name": "signer", "signer": true }
      ],
      "args": [
        { "name": "amount", "type": "u128" },
        { "name": "token_program_id", "type": "program_id" }
      ]
    }
  ]
}
```

### Use the IDL-Driven CLI

The CLI reads any program's IDL and auto-generates subcommands — no hand-written clap structs needed:

```bash
# See all available commands (auto-generated from IDL)
cargo run --bin treasury_cli -- --idl treasury-idl.json

# Output:
# 🔧 treasury_program v0.1.0 — IDL-driven CLI
#
# COMMANDS:
#   idl                    Print IDL information
#   create-vault         --token-name <[u8; 6]> --initial-supply <u128> ...
#   send                 --amount <u128> --token-program-id <program_id> --recipient-account <ID> --signer-account <ID>
#   deposit              --amount <u128> --token-program-id <program_id> --sender-holding-account <ID>
```

PDA accounts are auto-computed. Non-PDA accounts become `--*-account` flags:

```bash
# Send tokens from the treasury vault
cargo run --bin treasury_cli -- --idl treasury-idl.json send \
  --amount 100 \
  --token-program-id <TOKEN_PROGRAM_ID> \
  --recipient-account <RECIPIENT_ID> \
  --signer-account <YOUR_KEY>

# Output:
# 📋 Instruction: send
# Accounts:
#   📦 treasury_state_acct → auto-computed (PDA)
#   📦 vault_holding → auto-computed (PDA)
#   📦 recipient → <RECIPIENT_ID>
#   📦 signer → <YOUR_KEY>
# 🔧 Transaction: Send { amount: 100, token_program_id: ... }
```

Get per-instruction help:

```bash
cargo run --bin treasury_cli -- --idl treasury-idl.json send --help

# 📋 send — 4 account(s), 2 arg(s)
# ACCOUNTS:
#   treasury_state_acct (PDA — auto-computed)
#   vault_holding (PDA — auto-computed)
#   recipient
#   signer [signer]
# ARGS:
#   --amount                    amount (u128)
#   --token-program-id          token_program_id (program_id)
#   --recipient-account         Account ID for 'recipient'
#   --signer-account            Account ID for 'signer' [signer]
```

> **Note:** Transaction submission isn't wired up yet (needs a running sequencer). The CLI currently validates args and shows what it *would* build. See the [`bedrock-api` branch WORKSHOP.md](../../tree/bedrock-api/WORKSHOP.md) for running the full stack.

### Run (needs 3 terminals — see [WORKSHOP.md](../../tree/bedrock-api/WORKSHOP.md) on bedrock-api branch)

```bash
# Terminal 1: Logos Blockchain node
# Terminal 2: Indexer service  
# Terminal 3: Sequencer
# Then deploy + interact via wallet CLI or treasury_cli
```

## What This Proves

✅ The `#[nssa_program]` macro works on a **real program** with:
- PDA derivation and authorization
- Chained calls (cross-program invocation) with PDA seeds
- Account claiming (first-time init)
- Authorization checks (signer validation)
- Multiple instruction variants with different account layouts

✅ Framework types (`NssaOutput`, `NssaError`) bridge cleanly to real `nssa_core` types

✅ Generated `Instruction` enum uses serde (matching `read_nssa_inputs<T: DeserializeOwned>`)

## Roadmap

- [x] **`#[nssa_program]` macro** — auto-generates Instruction enum + main() from function signatures
- [x] **IDL generation** — `__program_idl()` function + `generate_idl` binary → JSON schema
- [x] **IDL-driven CLI** — `treasury_cli` reads IDL, auto-generates subcommands with PDA-aware args
- [ ] **Wire up transaction submission** — connect CLI to sequencer for actual on-chain execution
- [ ] **Account constraints** — `#[account(mut)]`, `#[account(init)]`, `#[account(signer)]` runtime validation
- [ ] **PDA derivation in macro** — `#[account(pda, seeds = [...])]` with automatic seed computation
- [ ] **Client SDK generation** — generate TypeScript/Python clients from IDL

## References

- [LSSA Repository](https://github.com/logos-blockchain/lssa)
- [Framework PoC (standalone)](../../tree/main/lssa-dx-poc) — original prototype with examples
- [Multisig branch](../../tree/multisig) — M-of-N threshold treasury + unified CLI
- [Workshop](../../tree/bedrock-api/WORKSHOP.md) — 60-min hands-on guide (bedrock-api branch)
