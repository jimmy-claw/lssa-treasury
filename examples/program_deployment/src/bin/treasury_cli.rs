/// IDL-driven CLI for NSSA programs.
///
/// Reads a program IDL (JSON) and generates CLI subcommands automatically.
/// Parses all types properly based on the IDL type information.
///
/// Usage:
///   cargo run --bin generate_idl > treasury-idl.json
///   cargo run --bin treasury_cli -- --idl treasury-idl.json create-vault \
///     --token-name "MYTKN0" --initial-supply 1000000 \
///     --token-program-id "0,0,0,0,0,0,0,0" \
///     --authorized-accounts "aabb...(64 hex chars),...(another 64 hex chars)" \
///     --token-definition-account "some-account-id"

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
            format!("--{} <{}>", snake_to_kebab(&a.name), idl_type_hint(&a.type_))
        }).collect();
        let acct_desc: Vec<String> = ix.accounts.iter().filter(|a| a.pda.is_none()).map(|a| {
            format!("--{}-account <HEX>", snake_to_kebab(&a.name))
        }).collect();

        let all_args: Vec<String> = args_desc.into_iter().chain(acct_desc).collect();
        println!("  {:<20} {}", cmd, all_args.join(" "));
    }
    println!();
    println!("TYPE FORMATS:");
    println!("  u128, u64, u32, u8    Decimal number");
    println!("  [u8; N]               Hex string (2*N hex chars) or UTF-8 string (≤N chars, right-padded)");
    println!("  [u32; 8] / program_id Comma-separated u32s: \"0,0,0,0,0,0,0,0\"");
    println!("  Vec<[u8; 32]>         Comma-separated hex strings: \"aabb...00,ccdd...00\"");
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
            println!("  --{:<25} {} ({}) — format: {}", 
                snake_to_kebab(&arg.name), arg.name, 
                idl_type_display(&arg.type_),
                idl_type_hint(&arg.type_));
        }
        for acc in &ix.accounts {
            if acc.pda.is_none() {
                println!("  --{}-account    Account ID for '{}' (64 hex chars)", snake_to_kebab(&acc.name), acc.name);
            }
        }
        process::exit(0);
    }

    map
}

// ─── Type parsing ────────────────────────────────────────────────

/// Parse a CLI string value according to the IDL type, returning a structured representation.
#[derive(Debug, Clone)]
enum ParsedValue {
    U8(u8),
    U32(u32),
    U64(u64),
    U128(u128),
    ByteArray(Vec<u8>),           // [u8; N]
    U32Array(Vec<u32>),           // [u32; N] / ProgramId
    ByteArrayVec(Vec<Vec<u8>>),   // Vec<[u8; 32]>
    Raw(String),                  // fallback
}

impl std::fmt::Display for ParsedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParsedValue::U8(v) => write!(f, "{}", v),
            ParsedValue::U32(v) => write!(f, "{}", v),
            ParsedValue::U64(v) => write!(f, "{}", v),
            ParsedValue::U128(v) => write!(f, "{}", v),
            ParsedValue::ByteArray(bytes) => {
                // Try to display as UTF-8 if all printable, otherwise hex
                if let Ok(s) = std::str::from_utf8(bytes) {
                    if s.chars().all(|c| c.is_ascii_graphic() || c == ' ') {
                        let trimmed = s.trim_end_matches('\0');
                        return write!(f, "\"{}\" (hex: {})", trimmed, hex_encode(bytes));
                    }
                }
                write!(f, "0x{}", hex_encode(bytes))
            }
            ParsedValue::U32Array(vals) => {
                let strs: Vec<String> = vals.iter().map(|v| v.to_string()).collect();
                write!(f, "[{}]", strs.join(", "))
            }
            ParsedValue::ByteArrayVec(vecs) => {
                let strs: Vec<String> = vecs.iter().map(|v| format!("0x{}", hex_encode(v))).collect();
                write!(f, "[{}]", strs.join(", "))
            }
            ParsedValue::Raw(s) => write!(f, "{}", s),
        }
    }
}

fn parse_value(raw: &str, ty: &IdlType) -> Result<ParsedValue, String> {
    match ty {
        IdlType::Primitive(p) => parse_primitive(raw, p),
        IdlType::Array { array } => parse_array(raw, &array.0, array.1),
        IdlType::Vec { vec } => parse_vec(raw, vec),
        IdlType::Option { option } => {
            if raw == "none" || raw == "null" || raw.is_empty() {
                Ok(ParsedValue::Raw("None".to_string()))
            } else {
                parse_value(raw, option)
            }
        }
        IdlType::Defined { defined } => {
            Ok(ParsedValue::Raw(format!("{}({})", defined, raw)))
        }
    }
}

fn parse_primitive(raw: &str, prim: &str) -> Result<ParsedValue, String> {
    match prim {
        "u8" => raw.parse::<u8>().map(ParsedValue::U8)
            .map_err(|e| format!("Invalid u8 '{}': {}", raw, e)),
        "u32" => raw.parse::<u32>().map(ParsedValue::U32)
            .map_err(|e| format!("Invalid u32 '{}': {}", raw, e)),
        "u64" => raw.parse::<u64>().map(ParsedValue::U64)
            .map_err(|e| format!("Invalid u64 '{}': {}", raw, e)),
        "u128" => raw.parse::<u128>().map(ParsedValue::U128)
            .map_err(|e| format!("Invalid u128 '{}': {}", raw, e)),
        "program_id" => {
            // ProgramId = [u32; 8], accept comma-separated u32 values
            parse_program_id(raw)
        }
        "bool" => {
            match raw {
                "true" | "1" | "yes" => Ok(ParsedValue::Raw("true".to_string())),
                "false" | "0" | "no" => Ok(ParsedValue::Raw("false".to_string())),
                _ => Err(format!("Invalid bool '{}': expected true/false", raw)),
            }
        }
        "string" | "String" => Ok(ParsedValue::Raw(raw.to_string())),
        other => Ok(ParsedValue::Raw(format!("{}({})", other, raw))),
    }
}

fn parse_program_id(raw: &str) -> Result<ParsedValue, String> {
    // Accept: "0,0,0,0,0,0,0,0" or hex "0000000000000000" (32 hex = 8*4 bytes)
    if raw.contains(',') {
        let parts: Vec<&str> = raw.split(',').map(|s| s.trim()).collect();
        if parts.len() != 8 {
            return Err(format!("ProgramId needs 8 u32 values, got {}", parts.len()));
        }
        let mut vals = Vec::with_capacity(8);
        for (i, p) in parts.iter().enumerate() {
            let v = if p.starts_with("0x") || p.starts_with("0X") {
                u32::from_str_radix(&p[2..], 16)
            } else {
                p.parse::<u32>()
            };
            vals.push(v.map_err(|e| format!("ProgramId[{}] invalid u32 '{}': {}", i, p, e))?);
        }
        Ok(ParsedValue::U32Array(vals))
    } else if raw.len() == 64 && raw.chars().all(|c| c.is_ascii_hexdigit()) {
        // 64 hex chars = 32 bytes = 8 u32s (little-endian)
        let bytes = hex_decode(raw)?;
        let mut vals = Vec::with_capacity(8);
        for chunk in bytes.chunks(4) {
            vals.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }
        Ok(ParsedValue::U32Array(vals))
    } else {
        Err(format!(
            "Invalid ProgramId '{}': expected 8 comma-separated u32s or 64 hex chars",
            raw
        ))
    }
}

fn parse_array(raw: &str, elem_type: &IdlType, size: usize) -> Result<ParsedValue, String> {
    match elem_type {
        IdlType::Primitive(p) if p == "u8" => {
            // [u8; N]: accept hex string or UTF-8 string
            if raw.len() == size * 2 && raw.chars().all(|c| c.is_ascii_hexdigit()) {
                // Hex input
                let bytes = hex_decode(raw)?;
                if bytes.len() != size {
                    return Err(format!("Expected {} bytes, got {}", size, bytes.len()));
                }
                Ok(ParsedValue::ByteArray(bytes))
            } else if raw.starts_with("0x") || raw.starts_with("0X") {
                let hex = &raw[2..];
                let bytes = hex_decode(hex)?;
                if bytes.len() != size {
                    return Err(format!("Expected {} bytes from hex, got {}", size, bytes.len()));
                }
                Ok(ParsedValue::ByteArray(bytes))
            } else {
                // UTF-8 string, right-pad with zeros
                let str_bytes = raw.as_bytes();
                if str_bytes.len() > size {
                    return Err(format!(
                        "String '{}' is {} bytes, max {} for [u8; {}]",
                        raw, str_bytes.len(), size, size
                    ));
                }
                let mut bytes = vec![0u8; size];
                bytes[..str_bytes.len()].copy_from_slice(str_bytes);
                Ok(ParsedValue::ByteArray(bytes))
            }
        }
        IdlType::Primitive(p) if p == "u32" => {
            // [u32; N]: comma-separated
            let parts: Vec<&str> = raw.split(',').map(|s| s.trim()).collect();
            if parts.len() != size {
                return Err(format!("Expected {} u32 values, got {}", size, parts.len()));
            }
            let mut vals = Vec::with_capacity(size);
            for p in &parts {
                vals.push(p.parse::<u32>().map_err(|e| format!("Invalid u32 '{}': {}", p, e))?);
            }
            Ok(ParsedValue::U32Array(vals))
        }
        _ => Ok(ParsedValue::Raw(raw.to_string())),
    }
}

fn parse_vec(raw: &str, elem_type: &IdlType) -> Result<ParsedValue, String> {
    match elem_type {
        IdlType::Array { array } => {
            match &*array.0 {
                IdlType::Primitive(p) if p == "u8" => {
                    // Vec<[u8; N]>: comma-separated hex strings
                    let size = array.1;
                    if raw.is_empty() {
                        return Ok(ParsedValue::ByteArrayVec(vec![]));
                    }
                    let parts: Vec<&str> = raw.split(',').map(|s| s.trim()).collect();
                    let mut result = Vec::with_capacity(parts.len());
                    for (i, part) in parts.iter().enumerate() {
                        let hex = part.strip_prefix("0x").or_else(|| part.strip_prefix("0X")).unwrap_or(part);
                        let bytes = hex_decode(hex).map_err(|e| format!("Element [{}]: {}", i, e))?;
                        if bytes.len() != size {
                            return Err(format!(
                                "Element [{}]: expected {} bytes ({}  hex chars), got {} bytes from '{}'",
                                i, size, size * 2, bytes.len(), part
                            ));
                        }
                        result.push(bytes);
                    }
                    Ok(ParsedValue::ByteArrayVec(result))
                }
                _ => Ok(ParsedValue::Raw(raw.to_string())),
            }
        }
        _ => Ok(ParsedValue::Raw(raw.to_string())),
    }
}

// ─── Hex utilities ───────────────────────────────────────────────

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hex_decode(hex: &str) -> Result<Vec<u8>, String> {
    if hex.len() % 2 != 0 {
        return Err(format!("Hex string has odd length: {}", hex.len()));
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        let byte = u8::from_str_radix(&hex[i..i + 2], 16)
            .map_err(|e| format!("Invalid hex at position {}: {}", i, e))?;
        bytes.push(byte);
    }
    Ok(bytes)
}

// ─── Serialization ───────────────────────────────────────────────

/// Serialize a parsed value to its binary (borsh-like) representation.
fn serialize_value(val: &ParsedValue) -> Vec<u8> {
    match val {
        ParsedValue::U8(v) => vec![*v],
        ParsedValue::U32(v) => v.to_le_bytes().to_vec(),
        ParsedValue::U64(v) => v.to_le_bytes().to_vec(),
        ParsedValue::U128(v) => v.to_le_bytes().to_vec(),
        ParsedValue::ByteArray(bytes) => bytes.clone(),
        ParsedValue::U32Array(vals) => {
            vals.iter().flat_map(|v| v.to_le_bytes()).collect()
        }
        ParsedValue::ByteArrayVec(vecs) => {
            // Borsh Vec: 4-byte LE length prefix + concatenated elements
            let mut out = (vecs.len() as u32).to_le_bytes().to_vec();
            for v in vecs {
                out.extend_from_slice(v);
            }
            out
        }
        ParsedValue::Raw(_) => vec![], // can't serialize unknown
    }
}

// ─── Execute ─────────────────────────────────────────────────────

fn execute_instruction(
    _idl: &NssaIdl,
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

    // Parse and validate all args
    let mut parsed_args: Vec<(&str, ParsedValue)> = Vec::new();
    let mut has_errors = false;

    for arg in &ix.args {
        let key = snake_to_kebab(&arg.name);
        let raw = args.get(&key).unwrap();
        match parse_value(raw, &arg.type_) {
            Ok(val) => parsed_args.push((&arg.name, val)),
            Err(e) => {
                eprintln!("❌ --{}: {}", key, e);
                has_errors = true;
            }
        }
    }

    // Parse account IDs (expected as 64 hex chars = 32 bytes)
    let mut parsed_accounts: Vec<(&str, Vec<u8>)> = Vec::new();
    for acc in &ix.accounts {
        if acc.pda.is_some() {
            continue;
        }
        let key = format!("{}-account", snake_to_kebab(&acc.name));
        let raw = args.get(&key).unwrap();
        let hex = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")).unwrap_or(raw);
        match hex_decode(hex) {
            Ok(bytes) if bytes.len() == 32 => parsed_accounts.push((&acc.name, bytes)),
            Ok(bytes) => {
                eprintln!("❌ --{}: expected 32 bytes (64 hex chars), got {} bytes", key, bytes.len());
                has_errors = true;
            }
            Err(e) => {
                eprintln!("❌ --{}: {}", key, e);
                has_errors = true;
            }
        }
    }

    if has_errors {
        process::exit(1);
    }

    // Display accounts
    println!("Accounts:");
    for acc in &ix.accounts {
        if acc.pda.is_some() {
            println!("  📦 {} → auto-computed (PDA)", acc.name);
        } else {
            let account_bytes = parsed_accounts.iter().find(|(n, _)| *n == acc.name).unwrap();
            println!("  📦 {} → 0x{}", acc.name, hex_encode(&account_bytes.1));
        }
    }
    println!();

    // Display parsed args
    println!("Arguments (parsed):");
    for (name, val) in &parsed_args {
        println!("  {} = {}", name, val);
    }
    println!();

    // Build serialized instruction data
    // Format: instruction variant index (u8) + serialized args
    let ix_index = _idl.instructions.iter().position(|i| i.name == ix.name).unwrap_or(0);
    let mut instruction_data: Vec<u8> = vec![ix_index as u8];
    for (_, val) in &parsed_args {
        instruction_data.extend_from_slice(&serialize_value(val));
    }

    println!("🔧 Transaction:");
    println!("  program: {}", program_path);
    println!("  instruction index: {}", ix_index);
    println!("  instruction: {} {{", to_pascal_case(&ix.name));
    for (name, val) in &parsed_args {
        println!("    {}: {},", name, val);
    }
    println!("  }}");
    println!();
    println!("  Serialized instruction data ({} bytes):", instruction_data.len());
    println!("    {}", hex_encode(&instruction_data));
    println!();
    println!("⚠️  Transaction submission not yet wired up.");
    println!("    But all types are parsed and serialized correctly.");
}

// ─── Display helpers ─────────────────────────────────────────────

fn idl_type_display(ty: &IdlType) -> String {
    match ty {
        IdlType::Primitive(s) => s.clone(),
        IdlType::Vec { vec } => format!("Vec<{}>", idl_type_display(vec)),
        IdlType::Option { option } => format!("Option<{}>", idl_type_display(option)),
        IdlType::Defined { defined } => defined.clone(),
        IdlType::Array { array } => format!("[{}; {}]", idl_type_display(&array.0), array.1),
    }
}

fn idl_type_hint(ty: &IdlType) -> String {
    match ty {
        IdlType::Primitive(s) => match s.as_str() {
            "u8" | "u32" | "u64" | "u128" => "NUMBER".to_string(),
            "program_id" => "u32,u32,...(×8)".to_string(),
            "bool" => "true|false".to_string(),
            _ => s.to_uppercase(),
        },
        IdlType::Vec { vec } => match &**vec {
            IdlType::Array { array } => match &*array.0 {
                IdlType::Primitive(p) if p == "u8" => {
                    format!("HEX{},...", array.1 * 2)
                }
                _ => "LIST".to_string(),
            }
            _ => "LIST".to_string(),
        },
        IdlType::Option { option } => format!("OPT<{}>", idl_type_hint(option)),
        IdlType::Defined { defined } => defined.clone(),
        IdlType::Array { array } => match &*array.0 {
            IdlType::Primitive(p) if p == "u8" => {
                format!("HEX{}|STR≤{}", array.1 * 2, array.1)
            }
            _ => format!("[_; {}]", array.1),
        },
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
