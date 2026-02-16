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
        eprintln!("Usage: multisig_execute <treasury.bin> <recipient_id> <amount> [<signer_key_file> ...]");
        eprintln!("Example: multisig_execute artifacts/treasury.bin <recipient> 1000");
        std::process::exit(1);
    }

    let treasury_path = &args[1];
    let recipient: AccountId = args[2].parse().expect("Invalid recipient account ID");
    let amount: u128 = args[3].parse().expect("Invalid amount");

    // Load treasury program
    let treasury_bytecode: Vec<u8> = std::fs::read(treasury_path).unwrap();
    let treasury_program = Program::new(treasury_bytecode).unwrap();
    let treasury_program_id = treasury_program.id();

    let multisig_state_id = compute_multisig_state_pda(&treasury_program_id);

    println!("Treasury program ID:    {:?}", treasury_program_id);
    println!("Multisig state PDA:     {}", multisig_state_id);
    println!("Recipient:              {}", recipient);
    println!("Amount:                 {}", amount);

    let instruction = Instruction::Execute {
        recipient: recipient.clone(),
        amount,
    };

    // Accounts: multisig state + recipient (for the transfer)
    let account_ids = vec![multisig_state_id, recipient];

    // Collect signer account IDs from remaining args
    let signer_ids: Vec<AccountId> = args[4..].iter()
        .map(|s| s.parse::<AccountId>().expect("Invalid signer account ID"))
        .collect();

    // Get nonces for signers
    let nonces = if !signer_ids.is_empty() {
        wallet_core.get_accounts_nonces(signer_ids.clone()).await
            .expect("Failed to get nonces")
    } else {
        vec![]
    };

    let message = Message::try_new(
        treasury_program_id,
        account_ids,
        nonces,
        instruction,
    ).unwrap();

    // For PoC: use single signer (first one)
    // Multi-signer witness aggregation would need offline signing rounds
    if signer_ids.is_empty() {
        eprintln!("Warning: no signers provided, submitting without signatures");
    }
    let signing_key = if !signer_ids.is_empty() {
        Some(wallet_core.storage().user_data
            .get_pub_account_signing_key(&signer_ids[0])
            .expect("Signer private key not found in wallet"))
    } else {
        None
    };
    let witness_set = match &signing_key {
        Some(key) => WitnessSet::for_message(&message, &[key]),
        None => WitnessSet::for_message(&message, &[] as &[&nssa::PrivateKey]),
    };

    let tx = PublicTransaction::new(message, witness_set);

    let response = wallet_core
        .sequencer_client
        .send_tx_public(tx)
        .await
        .unwrap();

    println!("\n📤 Execute transaction submitted!");
    println!("   Amount: {} → {}", amount, args[2]);
    println!("   tx_hash: {}", response.tx_hash);
    println!("   Waiting for confirmation...");

    let poller = wallet::poller::TxPoller::new(
        wallet_core.config().clone(),
        wallet_core.sequencer_client.clone(),
    );

    match poller.poll_tx(response.tx_hash).await {
        Ok(_) => println!("✅ Multisig execution confirmed!"),
        Err(e) => {
            eprintln!("❌ Transaction NOT confirmed: {e:#}");
            std::process::exit(1);
        }
    }
}
