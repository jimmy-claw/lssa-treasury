//! Example: Deploy the Treasury program and create a vault.
//!
//! Usage:
//!   cargo run --bin deploy_and_create_vault \
//!     <path/to/treasury.bin> \
//!     <treasury_state_account_id> \
//!     <token_definition_account_id> \
//!     <vault_holding_account_id>
//!
//! The account IDs for the PDAs should be computed off-chain using
//! `treasury_core::compute_treasury_state_pda` and
//! `treasury_core::compute_vault_holding_pda`.

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
    let token_def_id: AccountId = std::env::args_os()
        .nth(3).unwrap().into_string().unwrap().parse().unwrap();
    let vault_holding_id: AccountId = std::env::args_os()
        .nth(4).unwrap().into_string().unwrap().parse().unwrap();

    // Load the treasury program
    let bytecode: Vec<u8> = std::fs::read(&program_path).unwrap();
    let program = Program::new(bytecode).unwrap();

    println!("Treasury program ID: {:?}", program.id());

    // Build the CreateVault instruction
    // The token program ID needs to be known ahead of time (from deploying the token program)
    let token_program_id = [0u32; 8]; // TODO: replace with actual token program ID

    let instruction = treasury_core::Instruction::CreateVault {
        token_name: "TreasuryToken".to_string(),
        initial_supply: 1_000_000,
        treasury_program_id: program.id(),
        token_program_id,
    };

    let greeting = risc0_zkvm::serde::to_vec(&instruction).unwrap();
    let greeting_bytes: Vec<u8> = greeting.iter().flat_map(|w| w.to_le_bytes()).collect();

    // Build and submit the transaction
    let account_ids = vec![treasury_state_id, token_def_id, vault_holding_id];
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

    println!("CreateVault transaction submitted!");
}
