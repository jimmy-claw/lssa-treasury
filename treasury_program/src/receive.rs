//! Deposit/Receive instruction handler.
//!
//! Receives tokens from an external sender into the treasury vault PDA
//! by chaining to the Token program's `Transfer` instruction.
//!
//! Note: No PDA authorization is needed here because the *sender* is the one
//! who needs to be authorized (they're spending their tokens), and that
//! authorization comes from the user's signature, not from the treasury program.

use nssa_core::{
    account::AccountWithMetadata,
    program::{AccountPostState, ChainedCall, ProgramId},
};

/// Handle the `Deposit` instruction.
///
/// **Account layout (pre_states):**
/// 0. `treasury_state` — treasury state PDA (read-only here)
/// 1. `sender_holding` — sender's token holding (authorized by user signature)
/// 2. `vault_holding` — treasury vault PDA that receives the tokens
pub fn deposit(
    treasury_state: AccountWithMetadata,
    sender_holding: AccountWithMetadata,
    vault_holding: AccountWithMetadata,
    amount: u128,
    token_program_id: ProgramId,
) -> (Vec<AccountPostState>, Vec<ChainedCall>) {
    // Verify treasury_state is initialized
    assert!(
        treasury_state.account != nssa_core::account::Account::default(),
        "Treasury state is not initialized"
    );

    // The sender is already authorized by the user's signature in the transaction.
    // We just chain to Token::Transfer: sender → vault
    let chained_call = ChainedCall::new(
        token_program_id,
        vec![sender_holding.clone(), vault_holding.clone()],
        &token_core::Instruction::Transfer {
            amount_to_transfer: amount,
        },
    );

    // Post states: all unchanged — Token program handles mutations.
    let post_states = vec![
        AccountPostState::new(treasury_state.account.clone()),
        AccountPostState::new(sender_holding.account.clone()),
        AccountPostState::new(vault_holding.account.clone()),
    ];

    (post_states, vec![chained_call])
}
