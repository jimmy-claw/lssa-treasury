//! Example: Deploy the Treasury program - simple noop test.

use nssa::{
    AccountId, PublicTransaction,
    program::Program,
    public_transaction::{Message, WitnessSet},
};
use treasury_core::compute_treasury_state_pda;
use wallet::WalletCore;

#[tokio::main]
async fn main() {
    let wallet_core = WalletCore::from_env().unwrap();

    let treasury_bin_path = std::env::args_os()
        .nth(1)
        .expect("Usage: deploy_and_create_vault <treasury.bin> <token.bin> <token_def_account_id>")
        .into_string()
        .unwrap();
    let token_def_id: AccountId = std::env::args_os()
        .nth(2)
        .expect("Missing <token_definition_account_id>")
        .into_string()
        .unwrap()
        .parse()
        .unwrap();

    let treasury_bytecode: Vec<u8> = std::fs::read(&treasury_bin_path).unwrap();
    let treasury_program = Program::new(treasury_bytecode).unwrap();
    let treasury_program_id = treasury_program.id();

    let treasury_state_id = compute_treasury_state_pda(&treasury_program_id);

    println!("Treasury program ID:    {:?}", treasury_program_id);
    println!("Treasury state PDA:     {}", treasury_state_id);
    println!("Token definition:       {}", token_def_id);

    // Send empty instruction - just test execution
    let instruction: Vec<u8> = vec![];

    let account_ids = vec![treasury_state_id, token_def_id];
    let nonces = vec![];
    let signing_keys = [];
    let message = Message::try_new(treasury_program_id, account_ids, nonces, instruction).unwrap();
    let witness_set = WitnessSet::for_message(&message, &signing_keys);
    let tx = PublicTransaction::new(message, witness_set);

    let _response = wallet_core
        .sequencer_client
        .send_tx_public(tx)
        .await
        .unwrap();

    println!("\n✅ Test transaction submitted!");
}
