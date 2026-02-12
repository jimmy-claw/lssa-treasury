//! Example: Send tokens from the treasury vault to a recipient.
//!
//! Usage:
//!   cargo run --bin send_from_vault \
//!     <path/to/treasury.bin> \
//!     <treasury_state_account_id> \
//!     <vault_holding_account_id> \
//!     <recipient_account_id> \
//!     <amount>

use nssa::{
    AccountId, PublicTransaction,
    program::Program,
    public_transaction::{Message, WitnessSet},
};
use wallet::WalletCore;

#[tokio::main]
async fn main() {
    // Initialize wallet
    let wallet_core = WalletCore::from_env().unwrap();

    // Parse arguments
    let program_path = std::env::args_os().nth(1).unwrap().into_string().unwrap();
    let treasury_state_id: AccountId = std::env::args_os()
        .nth(2).unwrap().into_string().unwrap().parse().unwrap();
    let vault_holding_id: AccountId = std::env::args_os()
        .nth(3).unwrap().into_string().unwrap().parse().unwrap();
    let recipient_id: AccountId = std::env::args_os()
        .nth(4).unwrap().into_string().unwrap().parse().unwrap();
    let amount: u128 = std::env::args_os()
        .nth(5).unwrap().into_string().unwrap().parse().unwrap();

    // Load the treasury program
    let bytecode: Vec<u8> = std::fs::read(&program_path).unwrap();
    let program = Program::new(bytecode).unwrap();

    // Build the Send instruction
    let token_program_id = [0u32; 8]; // TODO: replace with actual token program ID

    let instruction = treasury_core::Instruction::Send {
        amount,
        token_program_id,
    };

    let greeting = risc0_zkvm::serde::to_vec(&instruction).unwrap();
    let greeting_bytes: Vec<u8> = greeting.iter().flat_map(|w| w.to_le_bytes()).collect();

    // Build and submit the transaction
    let account_ids = vec![treasury_state_id, vault_holding_id, recipient_id];
    let nonces = vec![];
    let signing_keys = [];
    let message = Message::try_new(program.id(), account_ids, nonces, greeting_bytes).unwrap();
    let witness_set = WitnessSet::for_message(&message, &signing_keys);
    let tx = PublicTransaction::new(message, witness_set);

    let _response = wallet_core
        .sequencer_client
        .send_tx_public(tx)
        .await
        .unwrap();

    println!("Send transaction submitted! Sent {} tokens from vault.", amount);
}
