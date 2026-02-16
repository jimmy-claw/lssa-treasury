/// IDL-driven CLI for NSSA programs.
///
/// Reads a program IDL (JSON) and generates CLI subcommands automatically.
/// No hand-written clap structs needed — the IDL IS the interface.
///
/// Usage:
///   cargo run --bin generate_idl > treasury-idl.json
///   cargo run --bin treasury_cli -- --idl treasury-idl.json create-vault \
///     --token-name "MYTKN" --initial-supply 1000000

use nssa_framework_core::idl::{IdlType, NssaIdl};
use std::{collections::HashMap, env, fs, process};

fn main() {
    let args: Vec<String> = env::args().collect();

    // Find --idl flag
    let mut idl_path = "treasury-idl.json".to_string();
    let mut program_path = "artifacts/treasury.bin".to_string();
    let mut remaining_args: Vec<String> = vec![args[0].clone()];
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--idl" | "-i" => {
                i += 1;
                if i < args.len() {
                    idl_path = args[i].clone();
                }
            }
            "--program" | "-p" => {
                i += 1;
                if i < args.len() {
                    program_path = args[i].clone();
                }
            }
            _ => remaining_args.push(args[i].clone()),
        }
        i += 1;
    }

    // Load IDL
    let idl_content = match fs::read_to_string(&idl_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading IDL '{}': {}", idl_path, e);
            eprintln!("Generate it: cargo run --bin generate_idl > {}", idl_path);
            process::exit(1);
        }
    };
    let idl: NssaIdl = serde_json::from_str(&idl_content).unwrap_or_else(|e| {
        eprintln!("Error parsing IDL: {}", e);
        process::exit(1);
    });

    // Get subcommand name
    let subcmd = remaining_args.get(1).map(|s| s.as_str());

    match subcmd {
        Some("--help") | Some("-h") | None => {
            print_help(&idl);
        }
        Some("idl") => {
            print_idl_info(&idl);
        }
        Some(cmd) => {
            // Find matching instruction
            let instruction = idl.instructions.iter().find(|ix| {
                snake_to_kebab(&ix.name) == cmd || ix.name == cmd
            });

            match instruction {
                Some(ix) => {
                    // Parse instruction args from remaining CLI args
                    let cli_args = parse_instruction_args(&remaining_args[2..], ix);
                    execute_instruction(&idl, ix, &cli_args, &program_path);
                }
                None => {
                    eprintln!("Unknown command: {}", cmd);
                    eprintln!();
                    print_help(&idl);
                    process::exit(1);
                }
            }
        }
    }
}

fn print_help(idl: &NssaIdl) {
    println!("🔧 {} v{} — IDL-driven CLI", idl.name, idl.version);
    println!();
    println!("USAGE:");
    println!("  treasury_cli [OPTIONS] <COMMAND> [ARGS]");
    println!();
    println!("OPTIONS:");
    println!("  -i, --idl <FILE>       IDL JSON file [default: treasury-idl.json]");
    println!("  -p, --program <FILE>   Program binary [default: artifacts/treasury.bin]");
    println!();
    println!("COMMANDS:");
    println!("  idl                    Print IDL information");

    for ix in &idl.instructions {
        let cmd = snake_to_kebab(&ix.name);
        let args_desc: Vec<String> = ix.args.iter().map(|a| {
            format!("--{} <{}>", snake_to_kebab(&a.name), idl_type_short(&a.type_))
        }).collect();
        let acct_desc: Vec<String> = ix.accounts.iter().filter(|a| a.pda.is_none()).map(|a| {
            format!("--{}-account <ID>", snake_to_kebab(&a.name))
        }).collect();

        let all_args: Vec<String> = args_desc.into_iter().chain(acct_desc).collect();
        println!("  {:<20} {}", cmd, all_args.join(" "));
    }
    println!();
    println!("Auto-generated from IDL. Accounts marked as PDA are computed automatically.");
}

fn print_idl_info(idl: &NssaIdl) {
    println!("{}", serde_json::to_string_pretty(idl).unwrap());
}

fn parse_instruction_args(args: &[String], ix: &nssa_framework_core::idl::IdlInstruction) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut i = 0;
    while i < args.len() {
        if args[i].starts_with("--") {
            let key = args[i][2..].to_string();
            if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                map.insert(key, args[i + 1].clone());
                i += 2;
            } else {
                map.insert(key, "true".to_string());
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    // Check for --help
    if map.contains_key("help") || map.contains_key("h") {
        println!("📋 {} — {} account(s), {} arg(s)", ix.name, ix.accounts.len(), ix.args.len());
        println!();
        println!("ACCOUNTS:");
        for acc in &ix.accounts {
            let mut flags = vec![];
            if acc.writable { flags.push("mut"); }
            if acc.signer { flags.push("signer"); }
            if acc.init { flags.push("init"); }
            let flags_str = if flags.is_empty() { String::new() } else { format!(" [{}]", flags.join(", ")) };
            let pda_note = if acc.pda.is_some() { " (PDA — auto-computed)" } else { "" };
            println!("  {}{}{}", acc.name, flags_str, pda_note);
        }
        println!();
        println!("ARGS:");
        for arg in &ix.args {
            println!("  --{:<25} {} ({})", snake_to_kebab(&arg.name), arg.name, idl_type_display(&arg.type_));
        }
        for acc in &ix.accounts {
            if acc.pda.is_none() {
                println!("  --{}-account    Account ID for '{}'", snake_to_kebab(&acc.name), acc.name);
            }
        }
        process::exit(0);
    }

    map
}

fn execute_instruction(
    idl: &NssaIdl,
    ix: &nssa_framework_core::idl::IdlInstruction,
    args: &HashMap<String, String>,
    program_path: &str,
) {
    println!("📋 Instruction: {}", ix.name);
    println!();

    // Validate all required args are present
    let mut missing = vec![];
    for arg in &ix.args {
        let key = snake_to_kebab(&arg.name);
        if !args.contains_key(&key) {
            missing.push(format!("--{}", key));
        }
    }
    for acc in &ix.accounts {
        if acc.pda.is_none() {
            let key = format!("{}-account", snake_to_kebab(&acc.name));
            if !args.contains_key(&key) {
                missing.push(format!("--{}", key));
            }
        }
    }

    if !missing.is_empty() {
        eprintln!("❌ Missing required arguments: {}", missing.join(", "));
        eprintln!();
        eprintln!("Run with --help for usage.");
        process::exit(1);
    }

    // Show accounts
    println!("Accounts:");
    for acc in &ix.accounts {
        if acc.pda.is_some() {
            println!("  📦 {} → auto-computed (PDA)", acc.name);
        } else {
            let key = format!("{}-account", snake_to_kebab(&acc.name));
            let value = args.get(&key).unwrap();
            println!("  📦 {} → {}", acc.name, value);
        }
    }
    println!();

    // Show args
    println!("Arguments:");
    for arg in &ix.args {
        let key = snake_to_kebab(&arg.name);
        let value = args.get(&key).unwrap();
        println!("  {} = {} ({})", arg.name, value, idl_type_display(&arg.type_));
    }
    println!();

    // Show what would be built
    println!("🔧 Transaction:");
    println!("  program: {}", program_path);
    println!("  instruction: {} {{", to_pascal_case(&ix.name));
    for arg in &ix.args {
        let key = snake_to_kebab(&arg.name);
        let value = args.get(&key).unwrap();
        println!("    {}: {},", arg.name, value);
    }
    println!("  }}");
    println!();
    println!("⚠️  Transaction submission not yet wired up.");
    println!("    This demonstrates IDL → CLI generation.");
}

fn idl_type_display(ty: &IdlType) -> String {
    match ty {
        IdlType::Primitive(s) => s.clone(),
        IdlType::Vec { vec } => format!("Vec<{}>", idl_type_display(vec)),
        IdlType::Option { option } => format!("Option<{}>", idl_type_display(option)),
        IdlType::Defined { defined } => defined.clone(),
        IdlType::Array { array } => format!("[{}; {}]", idl_type_display(&array.0), array.1),
    }
}

fn idl_type_short(ty: &IdlType) -> String {
    match ty {
        IdlType::Primitive(s) => s.clone(),
        IdlType::Vec { .. } => "LIST".to_string(),
        IdlType::Option { .. } => "OPT".to_string(),
        IdlType::Defined { defined } => defined.clone(),
        IdlType::Array { array } => format!("[{}; {}]", idl_type_short(&array.0), array.1),
    }
}

fn snake_to_kebab(s: &str) -> String {
    s.replace('_', "-")
}

fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(ch) => ch.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect()
}
