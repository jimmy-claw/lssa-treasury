//! CreateVault instruction handler.
//!
//! Creates/updates the treasury state PDA, then chains to the Token program's
//! `NewFungibleDefinition` instruction to mint the initial supply into the
//! treasury's vault holding PDA.

use nssa_core::{
    account::{Account, AccountWithMetadata, Data},
    program::{AccountPostState, ChainedCall, ProgramId},
};
use treasury_core::{TreasuryState, vault_holding_pda_seed};

/// Handle the `CreateVault` instruction.
///
/// **Account layout (pre_states):**
/// 0. `treasury_state` — treasury state PDA (claimed on first call)
/// 1. `token_definition` — uninitialized, will become the new token def (claimed by Token program)
/// 2. `vault_holding` — uninitialized, will receive minted tokens (claimed by Token program)
pub fn create_vault(
    treasury_state: AccountWithMetadata,
    token_definition: AccountWithMetadata,
    vault_holding: AccountWithMetadata,
    token_name: String,
    initial_supply: u128,
    _treasury_program_id: ProgramId,
    token_program_id: ProgramId,
) -> (Vec<AccountPostState>, Vec<ChainedCall>) {
    // -- 1. Update treasury state -----------------------------------------------
    let mut state: TreasuryState = if treasury_state.account == Account::default() {
        TreasuryState::default()
    } else {
        borsh::from_slice(treasury_state.account.data.as_ref())
            .expect("Failed to deserialize TreasuryState")
    };

    state.vault_count += 1;

    let mut treasury_post = treasury_state.account.clone();
    let state_bytes = borsh::to_vec(&state).expect("Failed to serialize TreasuryState");
    treasury_post.data = Data::try_from(state_bytes).expect("TreasuryState should fit in Data");

    let treasury_post_state = if treasury_state.account == Account::default() {
        AccountPostState::new_claimed(treasury_post)
    } else {
        AccountPostState::new(treasury_post)
    };

    // -- 2. Build chained call to Token::NewFungibleDefinition ------------------
    //
    // The Token program will:
    //   - Create the token definition (claims token_definition account)
    //   - Mint initial_supply into vault_holding (claims vault_holding account)
    //
    // We authorize the vault_holding PDA by providing its seed so the Token
    // program sees it as authorized when it tries to write.

    // Prepare the vault_holding with is_authorized = true for the chained call
    let mut vault_for_chain = vault_holding.clone();
    vault_for_chain.is_authorized = true;

    let chained_call = ChainedCall::new(
        token_program_id,
        vec![token_definition.clone(), vault_for_chain],
        &token_core::Instruction::NewFungibleDefinition {
            name: token_name,
            total_supply: initial_supply,
        },
    )
    .with_pda_seeds(vec![vault_holding_pda_seed(&token_definition.account_id)]);

    // -- 3. Return post states + chained calls ----------------------------------
    //
    // We only modify the treasury_state. The token_definition and vault_holding
    // are passed unchanged — the Token program will modify them in the chained call.
    let post_states = vec![
        treasury_post_state,
        AccountPostState::new(token_definition.account.clone()),
        AccountPostState::new(vault_holding.account.clone()),
    ];

    (post_states, vec![chained_call])
}
