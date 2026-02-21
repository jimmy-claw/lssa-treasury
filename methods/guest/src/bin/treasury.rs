//! Treasury guest binary — entry point for the Risc0 zkVM.

#![no_main]

use nssa_core::program::{read_nssa_inputs, write_nssa_outputs_with_chained_call, ProgramOutput};
use treasury_program;

risc0_zkvm::guest::entry!(main);

fn main() {
    // Read inputs from the zkVM environment
    let (program_input, instruction_words) = read_nssa_inputs::<treasury_program::Instruction>();

    // Clone for process since it needs mutable references
    let mut accounts = program_input.pre_states.clone();

    // Process the instruction
    let output = treasury_program::process(
        &mut accounts,
        &program_input.instruction,
    );

    // Write outputs back to the zkVM
    write_nssa_outputs_with_chained_call(
        instruction_words,
        program_input.pre_states,
        output.post_states,
        output.chained_calls,
    );
}
