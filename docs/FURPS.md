# Multisig PoC — FURPS Specification

> **Status:** Draft — 2026-02-16
> **Scope:** Public mode only, basic M-of-N threshold
> **Target:** LEZ Testnet

---

## Functionality (F)

### F1. Multisig Setup
- **F1.1**: Create multisig with M-of-N threshold (M ≤ N, 1 ≤ M ≤ 5, N ≤ 10 for PoC)
- **F1.2**: Members identified by LEZ public keys (Schnorr/BIP-340)
- **F1.3**: Configuration stored in multisig state account
- **F1.4**: Multisig owns a treasury vault (PDA)

### F2. Transaction Execution
- **F2.1**: Any member can propose a transaction
- **F2.2**: M distinct members must sign for execution
- **F2.3**: Single on-chain transaction with all signatures
- **F2.4**: Support native token (λ) transfers

### F3. Member Management
- **F3.1**: Add member (requires M current signatures)
- **F3.2**: Remove member (requires M current signatures)
- **F3.3**: Change threshold (requires M current signatures, must satisfy 1 ≤ M ≤ N)

---

## Usability (U)

### U1. CLI Commands
```
# Create 2-of-3 multisig
lez-wallet multisig create --threshold 2 --member <pk1> --member <pk2> --member <pk3>

# View multisig info
lez-wallet multisig info --account <multisig_id>

# Propose transfer
lez-wallet multisig propose --multisig <id> --to <recipient> --amount 100

# Sign proposal (for each member)
lez-wallet multisig sign --proposal <file> --output <signed_file>

# Execute (collects signatures and submits)
lez-wallet multisig execute --proposal <file>

# Add member
lez-wallet multisig add-member --multisig <id> --member <new_pk>

# Remove member
lez-wallet multisig remove-member --multisig <id> --member <pk>
```

---

## Reliability (R)

- **R1**: No funds move without M valid signatures from distinct members
- **R2**: Nonce-based replay protection
- **R3**: Clear error messages for insufficient signatures, invalid members

---

## Performance (P)

- **P1**: Single on-chain transaction for execution
- **P2**: O(M) signature verifications per transaction

---

## Supportability (S)

- **S1**: Unit tests for all instructions
- **S2**: Integration test: create → fund → propose → approve → execute

---

## Relationship to Existing lssa-treasury

The existing `lssa-treasury` code provides:
- PDA derivation helpers
- Chained call infrastructure to token programs
- Guest binary entry point

**This PoC extends it:**
1. Replace 1-of-N logic with M-of-N threshold checking
2. Add member management instructions
3. Add CLI commands for full workflow

---

## Implementation Plan

1. **Day 1**: Update `treasury_core` — add M-of-N instructions
2. **Day 1**: Update `treasury_program` — implement threshold verification
3. **Day 1**: Add CLI commands to wallet
4. **Day 1**: Integration test
