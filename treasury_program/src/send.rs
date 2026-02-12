//! Send instruction handler.
//!
//! Sends tokens from the treasury vault PDA to a recipient by chaining
//! to the Token program's `Transfer` instruction.
//!
//! The key PDA pattern: the treasury program is the *authority* of the vault
//! PDA, so it can set `is_authorized = true` and provide the PDA seed.
//! The Token program is the *owner* of the vault (it claimed it during
//! CreateVault), so it can actually mutate the balance.

use nssa_core::{
    account::AccountWithMetadata,
    program::{AccountPostState, ChainedCall, ProgramId},
};
use treasury_core::vault_holding_pda_seed;

/// Handle the `Send` instruction.
///
/// **Account layout (pre_states):**
/// 0. `treasury_state` — treasury state PDA (read-only here)
/// 1. `vault_holding` — PDA holding tokens (treasury is authority)
/// 2. `recipient_holding` — destination account
pub fn send(
    treasury_state: AccountWithMetadata,
    vault_holding: AccountWithMetadata,
    recipient_holding: AccountWithMetadata,
    amount: u128,
    token_program_id: ProgramId,
) -> (Vec<AccountPostState>, Vec<ChainedCall>) {
    // Verify treasury_state is initialized (not default)
    assert!(
        treasury_state.account != nssa_core::account::Account::default(),
        "Treasury state is not initialized"
    );

    // Get the token definition id from the vault holding to compute the PDA seed
    let vault_token_holding = token_core::TokenHolding::try_from(&vault_holding.account.data)
        .expect("Vault must be a valid TokenHolding");
    let definition_id = vault_token_holding.definition_id();

    // Authorize the vault PDA for the chained call to Token::Transfer
    let mut vault_for_chain = vault_holding.clone();
    vault_for_chain.is_authorized = true;

    let chained_call = ChainedCall::new(
        token_program_id,
        vec![vault_for_chain, recipient_holding.clone()],
        &token_core::Instruction::Transfer {
            amount_to_transfer: amount,
        },
    )
    .with_pda_seeds(vec![vault_holding_pda_seed(&definition_id)]);

    // Post states: all accounts unchanged from our perspective —
    // the Token program handles balance mutations in the chained call.
    let post_states = vec![
        AccountPostState::new(treasury_state.account.clone()),
        AccountPostState::new(vault_holding.account.clone()),
        AccountPostState::new(recipient_holding.account.clone()),
    ];

    (post_states, vec![chained_call])
}
