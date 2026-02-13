//! Treasury program — on-chain logic for PDA demonstration.
//! This version is a noop that just passes through accounts.

use nssa_core::account::AccountWithMetadata;
use nssa_core::program::ProgramOutput;

/// Simple pass-through that just returns accounts unchanged.
pub fn process(
    accounts: &mut [AccountWithMetadata],
    _instruction: &(),
) -> ProgramOutput {
    let post_states = accounts.iter()
        .map(|a| nssa_core::program::AccountPostState::new(a.account.clone()))
        .collect();
    
    ProgramOutput {
        instruction_data: vec![],
        pre_states: accounts.to_vec(),
        post_states,
        chained_calls: vec![],
    }
}
