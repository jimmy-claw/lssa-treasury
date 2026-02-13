//! Treasury guest binary — entry point for the Risc0 zkVM.

#![no_main]

use nssa_core::program::{read_nssa_inputs, write_nssa_outputs};

fn main() {
    // Read inputs - but don't try to deserialize instruction
    let (program_input, instruction_words) = read_nssa_inputs::<()>();
    
    // Just pass through the accounts unchanged
    let post_states = program_input.pre_states
        .iter()
        .map(|pre| nssa_core::program::AccountPostState::new(pre.account.clone()))
        .collect();
    
    write_nssa_outputs(instruction_words, program_input.pre_states, post_states);
}
