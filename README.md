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
```

### Run (needs 3 terminals — see [WORKSHOP.md](../../tree/bedrock-api/WORKSHOP.md) on bedrock-api branch)

```bash
# Terminal 1: Logos Blockchain node
# Terminal 2: Indexer service  
# Terminal 3: Sequencer
# Then deploy + interact via wallet CLI
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

- [ ] **Account constraints** — `#[account(mut)]`, `#[account(init)]`, `#[account(signer)]` runtime validation
- [ ] **IDL generation** — JSON schema from macro metadata → client SDK generation
- [ ] **CLI generation** — auto-generate CLI subcommands from IDL (like Anchor)
- [ ] **PDA derivation in macro** — `#[account(pda, seeds = [...])]` with automatic seed computation

## References

- [LSSA Repository](https://github.com/logos-blockchain/lssa)
- [Framework PoC (standalone)](../../tree/main/lssa-dx-poc) — original prototype with examples
- [Multisig branch](../../tree/multisig) — M-of-N threshold treasury + unified CLI
- [Workshop](../../tree/bedrock-api/WORKSHOP.md) — 60-min hands-on guide (bedrock-api branch)
