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
    if args.len() < 3 {
        eprintln!("Usage: multisig_manage <treasury.bin> <action> [args...]");
        eprintln!();
        eprintln!("Actions:");
        eprintln!("  add-member <member_id>         Add a new member");
        eprintln!("  remove-member <member_id>       Remove a member");
        eprintln!("  change-threshold <threshold>    Change M-of-N threshold");
        std::process::exit(1);
    }

    let treasury_path = &args[1];
    let action = &args[2];

    let treasury_bytecode: Vec<u8> = std::fs::read(treasury_path).unwrap();
    let treasury_program = Program::new(treasury_bytecode).unwrap();
    let treasury_program_id = treasury_program.id();
    let multisig_state_id = compute_multisig_state_pda(&treasury_program_id);

    let instruction = match action.as_str() {
        "add-member" => {
            let member_id: AccountId = args.get(3)
                .expect("Missing <member_id>")
                .parse().expect("Invalid member account ID");
            println!("Adding member: {}", member_id);
            Instruction::AddMember {
                new_member: *member_id.value(),
            }
        }
        "remove-member" => {
            let member_id: AccountId = args.get(3)
                .expect("Missing <member_id>")
                .parse().expect("Invalid member account ID");
            println!("Removing member: {}", member_id);
            Instruction::RemoveMember {
                member_to_remove: *member_id.value(),
            }
        }
        "change-threshold" => {
            let threshold: u8 = args.get(3)
                .expect("Missing <threshold>")
                .parse().expect("Invalid threshold");
            println!("Changing threshold to: {}", threshold);
            Instruction::ChangeThreshold {
                new_threshold: threshold,
            }
        }
        _ => {
            eprintln!("Unknown action: {}", action);
            std::process::exit(1);
        }
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

    println!("\n📤 Management transaction submitted!");
    println!("   tx_hash: {}", response.tx_hash);
    println!("   Waiting for confirmation...");

    let poller = wallet::poller::TxPoller::new(
        wallet_core.config().clone(),
        wallet_core.sequencer_client.clone(),
    );

    match poller.poll_tx(response.tx_hash).await {
        Ok(_) => println!("✅ Transaction confirmed!"),
        Err(e) => {
            eprintln!("❌ Transaction NOT confirmed: {e:#}");
            std::process::exit(1);
        }
    }
}
