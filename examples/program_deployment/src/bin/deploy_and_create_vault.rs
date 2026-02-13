//! Example: Deploy the Treasury program and create a vault.
//!
//! Usage:
//!   cargo run --bin deploy_and_create_vault \
//!     <path/to/treasury.bin> \
//!     <path/to/token.bin> \
//!     <token_definition_account_id>
//!
//! The treasury_state and vault_holding PDA account IDs are computed
//! automatically from the treasury program ID and token definition ID.
//!
//! The token_definition account should be created beforehand with:
//!   wallet account new public

use nssa::{
    AccountId, PublicTransaction,
    program::Program,
    public_transaction::{Message, WitnessSet},
};
use treasury_core::{compute_treasury_state_pda, compute_vault_holding_pda};
use wallet::WalletCore;

#[tokio::main]
async fn main() {
    // Initialize wallet
    let wallet_core = WalletCore::from_env().unwrap();

    // Parse arguments
    let treasury_bin_path = std::env::args_os()
        .nth(1)
        .expect("Usage: deploy_and_create_vault <treasury.bin> <token.bin> <token_def_account_id>")
        .into_string()
        .unwrap();
    let token_bin_path = std::env::args_os()
        .nth(2)
        .expect("Missing <token.bin> path")
        .into_string()
        .unwrap();
    let token_def_id: AccountId = std::env::args_os()
        .nth(3)
        .expect("Missing <token_definition_account_id>")
        .into_string()
        .unwrap()
        .parse()
        .unwrap();

    // Load both programs to get their IDs
    let treasury_bytecode: Vec<u8> = std::fs::read(&treasury_bin_path).unwrap();
    let treasury_program = Program::new(treasury_bytecode).unwrap();
    let treasury_program_id = treasury_program.id();

    let token_bytecode: Vec<u8> = std::fs::read(&token_bin_path).unwrap();
    let token_program = Program::new(token_bytecode).unwrap();
    let token_program_id = token_program.id();

    // Compute PDA account IDs automatically
    let treasury_state_id = compute_treasury_state_pda(&treasury_program_id);
    let vault_holding_id = compute_vault_holding_pda(&treasury_program_id, &token_def_id);

    println!("Treasury program ID:    {:?}", treasury_program_id);
    println!("Token program ID:       {:?}", token_program_id);
    println!("Treasury state PDA:     {}", treasury_state_id);
    println!("Token definition:       {}", token_def_id);
    println!("Vault holding PDA:      {}", vault_holding_id);

    // Build the CreateVault instruction
    let instruction = treasury_core::Instruction::CreateVault {
        token_name: "TreasuryToken".to_string(),
        initial_supply: 1_000_000,
        treasury_program_id,
        token_program_id,
    };

    let instruction_data = risc0_zkvm::serde::to_vec(&instruction).unwrap();
    let instruction_bytes: Vec<u8> = instruction_data
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();

    // Build and submit the transaction
    let account_ids = vec![treasury_state_id, token_def_id, vault_holding_id];
    let nonces = vec![];
    let signing_keys = [];
    let message = Message::try_new(
        treasury_program_id,
        account_ids,
        nonces,
        instruction_bytes,
    )
    .unwrap();
    let witness_set = WitnessSet::for_message(&message, &signing_keys);
    let tx = PublicTransaction::new(message, witness_set);

    let _response = wallet_core
        .sequencer_client
        .send_tx_public(tx)
        .await
        .unwrap();

    println!("\n✅ CreateVault transaction submitted!");
    println!("   Token '{}' with supply {} minted into vault.", "TreasuryToken", 1_000_000);
}
