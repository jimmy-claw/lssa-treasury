//! The Treasury Program — guest binary entry point.
//!
//! This is compiled to RISC-V and executed inside the Risc0 zkVM.

use nssa_core::program::{ProgramInput, read_nssa_inputs, write_nssa_outputs_with_chained_call};
use treasury_core::Instruction;

fn main() {
    let (
        ProgramInput {
            pre_states,
            instruction,
        },
        instruction_words,
    ) = read_nssa_inputs::<Instruction>();

    let pre_states_clone = pre_states.clone();

    let (post_states, chained_calls) = match instruction {
        Instruction::CreateVault {
            token_name,
            initial_supply,
            treasury_program_id,
            token_program_id,
        } => {
            let [treasury_state, token_definition, vault_holding] = pre_states
                .try_into()
                .expect("CreateVault requires exactly 3 accounts");
            treasury_program::create_vault::create_vault(
                treasury_state,
                token_definition,
                vault_holding,
                token_name,
                initial_supply,
                treasury_program_id,
                token_program_id,
            )
        }
        Instruction::Send {
            amount,
            token_program_id,
        } => {
            let [treasury_state, vault_holding, recipient_holding] = pre_states
                .try_into()
                .expect("Send requires exactly 3 accounts");
            treasury_program::send::send(
                treasury_state,
                vault_holding,
                recipient_holding,
                amount,
                token_program_id,
            )
        }
        Instruction::Deposit {
            amount,
            token_program_id,
        } => {
            let [treasury_state, sender_holding, vault_holding] = pre_states
                .try_into()
                .expect("Deposit requires exactly 3 accounts");
            treasury_program::receive::deposit(
                treasury_state,
                sender_holding,
                vault_holding,
                amount,
                token_program_id,
            )
        }
    };

    write_nssa_outputs_with_chained_call(
        instruction_words,
        pre_states_clone,
        post_states,
        chained_calls,
    );
}
