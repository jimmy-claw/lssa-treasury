//! Treasury program — on-chain logic for PDA demonstration with Token integration.

pub mod create_vault;
pub mod send;
pub mod deposit;

pub use treasury_core::Instruction;

use nssa_core::account::AccountWithMetadata;
use nssa_core::program::ProgramOutput;

/// Dispatch incoming instructions to their handlers.
pub fn process(
    accounts: &mut [AccountWithMetadata],
    instruction: &Instruction,
) -> ProgramOutput {
    match instruction {
        Instruction::CreateVault {
            token_name,
            initial_supply,
            token_program_id,
        } => create_vault::handle(accounts, token_name, *initial_supply, token_program_id),
        Instruction::Send { amount, token_program_id } => send::handle(accounts, *amount, token_program_id),
        Instruction::Deposit { amount, token_program_id } => deposit::handle(accounts, *amount, token_program_id),
    }
}
