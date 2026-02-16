use nssa::{
    AccountId, PublicTransaction,
    program::Program,
    public_transaction::{Message, WitnessSet},
};
use treasury_core::{Instruction, compute_multisig_state_pda};
use wallet::WalletCore;

#[tokio::main]
async fn main() {
    let wallet_core = WalletCore::from_env().unwrap();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: create_multisig <treasury.bin> <threshold> <member_id> [<member_id> ...]");
        eprintln!("Example: create_multisig artifacts/treasury.bin 2 <pk1> <pk2> <pk3>");
        std::process::exit(1);
    }

    let treasury_path = &args[1];
    let threshold: u8 = args[2].parse().expect("Invalid threshold");
    let member_ids: Vec<AccountId> = args[3..]
        .iter()
        .map(|s| s.parse::<AccountId>().expect("Invalid member account ID"))
        .collect();

    if (threshold as usize) > member_ids.len() {
        eprintln!("Error: threshold ({}) > number of members ({})", threshold, member_ids.len());
        std::process::exit(1);
    }

    // Load treasury program
    let treasury_bytecode: Vec<u8> = std::fs::read(treasury_path).unwrap();
    let treasury_program = Program::new(treasury_bytecode).unwrap();
    let treasury_program_id = treasury_program.id();

    // Compute PDA
    let multisig_state_id = compute_multisig_state_pda(&treasury_program_id);

    println!("Treasury program ID:    {:?}", treasury_program_id);
    println!("Multisig state PDA:     {}", multisig_state_id);
    println!("Threshold:              {}/{}", threshold, member_ids.len());
    println!("Members:");
    for (i, m) in member_ids.iter().enumerate() {
        println!("  [{}] {}", i, m);
    }

    let members: Vec<[u8; 32]> = member_ids.iter()
        .map(|id| *id.value())
        .collect();

    let instruction = Instruction::CreateMultisig {
        threshold,
        members,
    };

    let account_ids = vec![multisig_state_id];
    let nonces = vec![];
    let signing_keys = [];
    let message = Message::try_new(
        treasury_program_id,
        account_ids,
        nonces,
        instruction,
    ).unwrap();
    let witness_set = WitnessSet::for_message(&message, &signing_keys);
    let tx = PublicTransaction::new(message, witness_set);

    let response = wallet_core
        .sequencer_client
        .send_tx_public(tx)
        .await
        .unwrap();

    println!("\n📤 CreateMultisig transaction submitted!");
    println!("   {}-of-{} multisig", threshold, member_ids.len());
    println!("   tx_hash: {}", response.tx_hash);
    println!("   Waiting for confirmation...");

    let poller = wallet::poller::TxPoller::new(
        wallet_core.config().clone(),
        wallet_core.sequencer_client.clone(),
    );

    match poller.poll_tx(response.tx_hash).await {
        Ok(_) => println!("✅ Multisig created and confirmed!"),
        Err(e) => {
            eprintln!("❌ Transaction NOT confirmed: {e:#}");
            std::process::exit(1);
        }
    }
}
