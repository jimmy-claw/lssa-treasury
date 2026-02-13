// treasury_core — shared types and PDA derivation helpers for the Treasury program.
//
//! This crate contains the instruction enum, vault state, and PDA seed helpers
//! used by both the on-chain guest program and off-chain tooling.

use borsh::{BorshDeserialize, BorshSerialize};
use nssa_core::account::AccountId;
use nssa_core::program::{PdaSeed, ProgramId};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Instructions
// ---------------------------------------------------------------------------

/// Instructions that the Treasury program understands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Instruction {
    /// Create a new vault for a token.
    ///
    /// Chains to Token::NewFungibleDefinition to create a new token definition
    /// and mint the initial supply into the treasury's PDA vault.
    ///
    /// **Accounts (in order):**
    /// 0. `treasury_state` — PDA owned by this program
    /// 1. `token_definition` — PDA for the new token definition
    /// 2. `vault_holding` — PDA that will hold the minted tokens
    CreateVault {
        token_name: String,
        initial_supply: u128,
        treasury_program_id: ProgramId,
        token_program_id: ProgramId,
    },

    /// Send tokens from the treasury vault to a recipient.
    ///
    /// **Accounts (in order):**
    /// 0. `treasury_state` — treasury state PDA
    /// 1. `vault_holding` — PDA holding tokens (authorized by treasury)
    /// 2. `recipient_holding` — destination account
    Send {
        amount: u128,
        token_program_id: ProgramId,
    },

    /// Deposit tokens into the treasury vault from an external sender.
    ///
    /// **Accounts (in order):**
    /// 0. `treasury_state` — treasury state PDA
    /// 1. `sender_holding` — sender's token holding (authorized by the user)
    /// 2. `vault_holding` — PDA that will receive the tokens
    Deposit {
        amount: u128,
        token_program_id: ProgramId,
    },
}

// ---------------------------------------------------------------------------
// Vault state (persisted in the treasury_state PDA)
// ---------------------------------------------------------------------------

/// Minimal state kept in the treasury PDA.
///
/// In a production program you would track additional metadata such as an
/// admin key, per-vault balances, etc.  For this example we keep it simple.
#[derive(Debug, Clone, Default, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct TreasuryState {
    /// How many vaults have been created through this treasury.
    pub vault_count: u64,
}

// ---------------------------------------------------------------------------
// PDA derivation helpers
// ---------------------------------------------------------------------------

/// Fixed 32-byte seed used to derive the treasury state PDA.
/// Padded with zeroes to fill 32 bytes.
const TREASURY_STATE_SEED: [u8; 32] = {
    let mut seed = [0u8; 32];
    let tag = b"treasury_state";
    let mut i = 0;
    while i < tag.len() {
        seed[i] = tag[i];
        i += 1;
    }
    seed
};

/// Compute the treasury state PDA account ID.
///
/// `account_id = PDA(treasury_program_id, TREASURY_STATE_SEED)`
pub fn compute_treasury_state_pda(treasury_program_id: &ProgramId) -> AccountId {
    AccountId::from((treasury_program_id, &treasury_state_pda_seed()))
}

/// Compute the vault holding PDA account ID for a given token definition.
///
/// `account_id = PDA(treasury_program_id, token_definition_id_bytes)`
pub fn compute_vault_holding_pda(
    treasury_program_id: &ProgramId,
    token_definition_id: &AccountId,
) -> AccountId {
    AccountId::from((treasury_program_id, &vault_holding_pda_seed(token_definition_id)))
}

/// Build the [`PdaSeed`] for the treasury state PDA so it can be provided
/// in chained calls for authorization.
pub fn treasury_state_pda_seed() -> PdaSeed {
    PdaSeed::new(TREASURY_STATE_SEED)
}

/// Build the [`PdaSeed`] for a vault holding PDA.
/// Uses the token definition's AccountId bytes as the seed.
pub fn vault_holding_pda_seed(token_definition_id: &AccountId) -> PdaSeed {
    PdaSeed::new(*token_definition_id.value())
}
