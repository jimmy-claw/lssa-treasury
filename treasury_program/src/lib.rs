// treasury_program — handler modules (legacy, pre-framework).
//
// With the nssa-framework macros, these handlers are now defined
// directly in the guest binary using #[nssa_program].
// This crate is kept for reference and test compatibility.

pub mod create_vault;
pub mod deposit;
pub mod send;

use nssa_core::account::AccountWithMetadata;
use nssa_core::program::{AccountPostState, ChainedCall};
use treasury_core::Instruction;

/// Main entry point (legacy pattern — replaced by #[nssa_program] macro).
pub fn process(
    accounts: &[AccountWithMetadata],
    instruction: &Instruction,
) -> (Vec<AccountPostState>, Vec<ChainedCall>) {
    match instruction {
        Instruction::CreateVault {
            token_name,
            initial_supply,
            token_program_id,
            authorized_accounts,
        } => create_vault::handle(accounts, token_name, *initial_supply, token_program_id, authorized_accounts),
        Instruction::Send {
            amount,
            token_program_id,
        } => send::handle(accounts, *amount, token_program_id),
        Instruction::Deposit {
            amount,
            token_program_id,
        } => deposit::handle(accounts, *amount, token_program_id),
    }
}
