/// IDL-driven CLI for NSSA programs.
///
/// Reads a program IDL (JSON) and generates CLI subcommands automatically.
/// Parses all types properly based on the IDL type information.
/// Supports risc0-compatible serialization and transaction submission.
///
/// Usage:
///   cargo run --bin generate_idl > treasury-idl.json
///   cargo run --bin treasury_cli -- --idl treasury-idl.json create-vault \
///     --token-name "MYTKN0" --initial-supply 1000000 \
///     --token-program-id "0,0,0,0,0,0,0,0" \
///     --authorized-accounts "aabb...(64 hex chars),...(another 64 hex chars)" \
///     --token-definition-account "some-account-id"
///
///   # Dry run (no submission):
///   cargo run --bin treasury_cli -- --dry-run --idl treasury-idl.json \
///     create-vault --token-name "MYTKN0" --initial-supply 1000000 \
///     --token-program-id "0,0,0,0,0,0,0,0" \
///     --authorized-accounts "aabb...00,ccdd...00" \
///     --token-definition-account "deadbeef...00"
///
///   # Submit (default — requires --treasury-bin and --token-bin):
///   cargo run --bin treasury_cli -- --idl treasury-idl.json \
///     --treasury-bin artifacts/treasury.bin --token-bin artifacts/token.bin \
///     create-vault --token-name "MYTKN0" --initial-supply 1000000 \
///     --token-program-id "0,0,0,0,0,0,0,0" \
///     --authorized-accounts "aabb...00,ccdd...00" \
///     --token-definition-account "deadbeef...00"

use nssa::program::Program;
use nssa::public_transaction::{Message, WitnessSet};
use nssa::{AccountId, PublicTransaction};
use nssa_core::program::{PdaSeed, ProgramId};
use nssa_framework_core::idl::{IdlSeed, IdlType, NssaIdl};
use std::{collections::HashMap, env, fs, process};
use wallet::WalletCore;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    // Find global flags
    let mut idl_path = "treasury-idl.json".to_string();
    let mut program_path = "artifacts/treasury.bin".to_string();
    let mut dry_run = false;
    let mut treasury_bin: Option<String> = None;
    let mut token_bin: Option<String> = None;
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
            "--dry-run" => {
                dry_run = true;
            }
            "--treasury-bin" => {
                i += 1;
                if i < args.len() {
                    treasury_bin = Some(args[i].clone());
                }
            }
            "--token-bin" => {
                i += 1;
                if i < args.len() {
                    token_bin = Some(args[i].clone());
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
                    execute_instruction(
                        &idl,
                        ix,
                        &cli_args,
                        &program_path,
                        dry_run,
                        treasury_bin.as_deref(),
                        token_bin.as_deref(),
                    )
                    .await;
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
    println!("  -i, --idl <FILE>           IDL JSON file [default: treasury-idl.json]");
    println!("  -p, --program <FILE>       Program binary [default: artifacts/treasury.bin]");
    println!("  --dry-run                  Print parsed/serialized data without submitting");
    println!("  --treasury-bin <FILE>      Treasury program binary (for submission)");
    println!("  --token-bin <FILE>         Token program binary (for submission)");
    println!();
    println!("COMMANDS:");
    println!("  idl                        Print IDL information");

    for ix in &idl.instructions {
        let cmd = snake_to_kebab(&ix.name);
        let args_desc: Vec<String> = ix
            .args
            .iter()
            .map(|a| format!("--{} <{}>", snake_to_kebab(&a.name), idl_type_hint(&a.type_)))
            .collect();
        let acct_desc: Vec<String> = ix
            .accounts
            .iter()
            .filter(|a| a.pda.is_none())
            .map(|a| format!("--{}-account <HEX>", snake_to_kebab(&a.name)))
            .collect();

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

fn parse_instruction_args(
    args: &[String],
    ix: &nssa_framework_core::idl::IdlInstruction,
) -> HashMap<String, String> {
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
        println!(
            "📋 {} — {} account(s), {} arg(s)",
            ix.name,
            ix.accounts.len(),
            ix.args.len()
        );
        println!();
        println!("ACCOUNTS:");
        for acc in &ix.accounts {
            let mut flags = vec![];
            if acc.writable {
                flags.push("mut");
            }
            if acc.signer {
                flags.push("signer");
            }
            if acc.init {
                flags.push("init");
            }
            let flags_str = if flags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", flags.join(", "))
            };
            let pda_note = if acc.pda.is_some() {
                " (PDA — auto-computed)"
            } else {
                ""
            };
            println!("  {}{}{}", acc.name, flags_str, pda_note);
        }
        println!();
        println!("ARGS:");
        for arg in &ix.args {
            println!(
                "  --{:<25} {} ({}) — format: {}",
                snake_to_kebab(&arg.name),
                arg.name,
                idl_type_display(&arg.type_),
                idl_type_hint(&arg.type_)
            );
        }
        for acc in &ix.accounts {
            if acc.pda.is_none() {
                println!(
                    "  --{}-account    Account ID for '{}' (64 hex chars)",
                    snake_to_kebab(&acc.name),
                    acc.name
                );
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
    Bool(bool),
    U8(u8),
    U32(u32),
    U64(u64),
    U128(u128),
    Str(String),
    ByteArray(Vec<u8>),         // [u8; N]
    U32Array(Vec<u32>),         // [u32; N] / ProgramId
    ByteArrayVec(Vec<Vec<u8>>), // Vec<[u8; 32]>
    None,                       // Option::None
    Some(Box<ParsedValue>),     // Option::Some
    Raw(String),                // fallback
}

impl std::fmt::Display for ParsedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParsedValue::Bool(v) => write!(f, "{}", v),
            ParsedValue::U8(v) => write!(f, "{}", v),
            ParsedValue::U32(v) => write!(f, "{}", v),
            ParsedValue::U64(v) => write!(f, "{}", v),
            ParsedValue::U128(v) => write!(f, "{}", v),
            ParsedValue::Str(s) => write!(f, "\"{}\"", s),
            ParsedValue::ByteArray(bytes) => {
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
            ParsedValue::None => write!(f, "None"),
            ParsedValue::Some(inner) => write!(f, "Some({})", inner),
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
                Ok(ParsedValue::None)
            } else {
                Ok(ParsedValue::Some(Box::new(parse_value(raw, option)?)))
            }
        }
        IdlType::Defined { defined } => Ok(ParsedValue::Raw(format!("{}({})", defined, raw))),
    }
}

fn parse_primitive(raw: &str, prim: &str) -> Result<ParsedValue, String> {
    match prim {
        "u8" => raw
            .parse::<u8>()
            .map(ParsedValue::U8)
            .map_err(|e| format!("Invalid u8 '{}': {}", raw, e)),
        "u32" => raw
            .parse::<u32>()
            .map(ParsedValue::U32)
            .map_err(|e| format!("Invalid u32 '{}': {}", raw, e)),
        "u64" => raw
            .parse::<u64>()
            .map(ParsedValue::U64)
            .map_err(|e| format!("Invalid u64 '{}': {}", raw, e)),
        "u128" => raw
            .parse::<u128>()
            .map(ParsedValue::U128)
            .map_err(|e| format!("Invalid u128 '{}': {}", raw, e)),
        "program_id" => parse_program_id(raw),
        "bool" => match raw {
            "true" | "1" | "yes" => Ok(ParsedValue::Bool(true)),
            "false" | "0" | "no" => Ok(ParsedValue::Bool(false)),
            _ => Err(format!("Invalid bool '{}': expected true/false", raw)),
        },
        "string" | "String" => Ok(ParsedValue::Str(raw.to_string())),
        other => Ok(ParsedValue::Raw(format!("{}({})", other, raw))),
    }
}

fn parse_program_id(raw: &str) -> Result<ParsedValue, String> {
    if raw.contains(',') {
        let parts: Vec<&str> = raw.split(',').map(|s| s.trim()).collect();
        if parts.len() != 8 {
            return Err(format!(
                "ProgramId needs 8 u32 values, got {}",
                parts.len()
            ));
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
            if raw.len() == size * 2 && raw.chars().all(|c| c.is_ascii_hexdigit()) {
                let bytes = hex_decode(raw)?;
                if bytes.len() != size {
                    return Err(format!("Expected {} bytes, got {}", size, bytes.len()));
                }
                Ok(ParsedValue::ByteArray(bytes))
            } else if raw.starts_with("0x") || raw.starts_with("0X") {
                let hex = &raw[2..];
                let bytes = hex_decode(hex)?;
                if bytes.len() != size {
                    return Err(format!(
                        "Expected {} bytes from hex, got {}",
                        size,
                        bytes.len()
                    ));
                }
                Ok(ParsedValue::ByteArray(bytes))
            } else {
                let str_bytes = raw.as_bytes();
                if str_bytes.len() > size {
                    return Err(format!(
                        "String '{}' is {} bytes, max {} for [u8; {}]",
                        raw,
                        str_bytes.len(),
                        size,
                        size
                    ));
                }
                let mut bytes = vec![0u8; size];
                bytes[..str_bytes.len()].copy_from_slice(str_bytes);
                Ok(ParsedValue::ByteArray(bytes))
            }
        }
        IdlType::Primitive(p) if p == "u32" => {
            let parts: Vec<&str> = raw.split(',').map(|s| s.trim()).collect();
            if parts.len() != size {
                return Err(format!("Expected {} u32 values, got {}", size, parts.len()));
            }
            let mut vals = Vec::with_capacity(size);
            for p in &parts {
                vals.push(
                    p.parse::<u32>()
                        .map_err(|e| format!("Invalid u32 '{}': {}", p, e))?,
                );
            }
            Ok(ParsedValue::U32Array(vals))
        }
        _ => Ok(ParsedValue::Raw(raw.to_string())),
    }
}

fn parse_vec(raw: &str, elem_type: &IdlType) -> Result<ParsedValue, String> {
    match elem_type {
        IdlType::Array { array } => match &*array.0 {
            IdlType::Primitive(p) if p == "u8" => {
                let size = array.1;
                if raw.is_empty() {
                    return Ok(ParsedValue::ByteArrayVec(vec![]));
                }
                let parts: Vec<&str> = raw.split(',').map(|s| s.trim()).collect();
                let mut result = Vec::with_capacity(parts.len());
                for (i, part) in parts.iter().enumerate() {
                    let hex = part
                        .strip_prefix("0x")
                        .or_else(|| part.strip_prefix("0X"))
                        .unwrap_or(part);
                    let bytes =
                        hex_decode(hex).map_err(|e| format!("Element [{}]: {}", i, e))?;
                    if bytes.len() != size {
                        return Err(format!(
                            "Element [{}]: expected {} bytes ({} hex chars), got {} bytes from '{}'",
                            i,
                            size,
                            size * 2,
                            bytes.len(),
                            part
                        ));
                    }
                    result.push(bytes);
                }
                Ok(ParsedValue::ByteArrayVec(result))
            }
            _ => Ok(ParsedValue::Raw(raw.to_string())),
        },
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

// ─── risc0 Serialization ────────────────────────────────────────

/// Serialize an instruction to risc0 serde format (Vec<u32>).
///
/// Matches the output of `risc0_zkvm::serde::to_vec` for an enum struct variant:
///   variant_index (u32), then each field serialized in order.
fn serialize_to_risc0(
    variant_index: u32,
    parsed_args: &[(&IdlType, &ParsedValue)],
) -> Vec<u32> {
    let mut out = vec![variant_index];
    for (ty, val) in parsed_args {
        serialize_value_risc0(&mut out, ty, val);
    }
    out
}

fn serialize_value_risc0(out: &mut Vec<u32>, ty: &IdlType, val: &ParsedValue) {
    match (ty, val) {
        (IdlType::Primitive(p), _) => serialize_primitive_risc0(out, p.as_str(), val),
        (IdlType::Array { array }, _) => serialize_array_risc0(out, &array.0, array.1, val),
        (IdlType::Vec { vec }, _) => serialize_vec_risc0(out, vec, val),
        (IdlType::Option { option: _ }, ParsedValue::None) => {
            // Option::None = variant 0
            out.push(0);
        }
        (IdlType::Option { option }, ParsedValue::Some(inner)) => {
            // Option::Some = variant 1, then the value
            out.push(1);
            serialize_value_risc0(out, option, inner);
        }
        (IdlType::Option { option }, _) => {
            // Non-None value passed directly (backwards compat)
            out.push(1);
            serialize_value_risc0(out, option, val);
        }
        _ => {
            // Defined / Raw — best effort, skip
            eprintln!(
                "⚠️  Cannot serialize Defined/Raw type in risc0 format: {:?}",
                val
            );
        }
    }
}

fn serialize_primitive_risc0(out: &mut Vec<u32>, prim: &str, val: &ParsedValue) {
    match (prim, val) {
        ("bool", ParsedValue::Bool(b)) => {
            // bool → u8 → u32
            out.push(if *b { 1 } else { 0 });
        }
        ("u8", ParsedValue::U8(v)) => {
            out.push(*v as u32);
        }
        ("u32", ParsedValue::U32(v)) => {
            out.push(*v);
        }
        ("u64", ParsedValue::U64(v)) => {
            // u64 → 2 words (lo, hi)
            out.push(*v as u32);
            out.push((*v >> 32) as u32);
        }
        ("u128", ParsedValue::U128(v)) => {
            // u128 → 16 LE bytes → write_padded_bytes → 4 u32 LE words
            let bytes = v.to_le_bytes(); // 16 bytes
            for chunk in bytes.chunks(4) {
                out.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
            }
        }
        ("program_id", ParsedValue::U32Array(vals)) => {
            // ProgramId = [u32; 8] — serde tuple → 8 words
            for v in vals {
                out.push(*v);
            }
        }
        ("string" | "String", ParsedValue::Str(s)) => {
            // String → length u32, then bytes padded to u32 boundary
            let bytes = s.as_bytes();
            out.push(bytes.len() as u32);
            serialize_bytes_padded(out, bytes);
        }
        _ => {
            eprintln!(
                "⚠️  Type mismatch in risc0 serialization: prim={}, val={:?}",
                prim, val
            );
        }
    }
}

fn serialize_array_risc0(out: &mut Vec<u32>, elem_type: &IdlType, _size: usize, val: &ParsedValue) {
    match (elem_type, val) {
        (IdlType::Primitive(p), ParsedValue::ByteArray(bytes)) if p == "u8" => {
            // [u8; N] as serde tuple → N words, each byte as u32
            for b in bytes {
                out.push(*b as u32);
            }
        }
        (IdlType::Primitive(p), ParsedValue::U32Array(vals)) if p == "u32" => {
            // [u32; N] as serde tuple → N words
            for v in vals {
                out.push(*v);
            }
        }
        _ => {
            eprintln!("⚠️  Cannot serialize array type in risc0 format: {:?}", val);
        }
    }
}

fn serialize_vec_risc0(out: &mut Vec<u32>, elem_type: &IdlType, val: &ParsedValue) {
    match (elem_type, val) {
        (IdlType::Array { array }, ParsedValue::ByteArrayVec(vecs)) => {
            // Vec<[u8; N]> → length, then each element as serde tuple
            out.push(vecs.len() as u32);
            match &*array.0 {
                IdlType::Primitive(p) if p == "u8" => {
                    for v in vecs {
                        for b in v {
                            out.push(*b as u32);
                        }
                    }
                }
                _ => {
                    eprintln!("⚠️  Cannot serialize Vec element type in risc0 format");
                }
            }
        }
        _ => {
            eprintln!("⚠️  Cannot serialize Vec type in risc0 format: {:?}", val);
        }
    }
}

/// Write bytes padded to u32 boundary (for String serialization).
fn serialize_bytes_padded(out: &mut Vec<u32>, bytes: &[u8]) {
    // Pack bytes into u32 words, little-endian, zero-padding the last word
    let mut i = 0;
    while i < bytes.len() {
        let remaining = bytes.len() - i;
        let mut word_bytes = [0u8; 4];
        let take = remaining.min(4);
        word_bytes[..take].copy_from_slice(&bytes[i..i + take]);
        out.push(u32::from_le_bytes(word_bytes));
        i += 4;
    }
}

// ─── PDA computation from IDL seeds ─────────────────────────────

/// Compute PDA AccountId from IDL seed definitions.
fn compute_pda_from_seeds(
    seeds: &[IdlSeed],
    program_id: &ProgramId,
    account_map: &HashMap<String, AccountId>,
    _parsed_args: &HashMap<String, ParsedValue>,
) -> Result<AccountId, String> {
    // For now we support single-seed PDAs (which is what the treasury uses)
    // Each seed produces a 32-byte value; if multiple seeds, we'd need to combine them.
    // The treasury IDL uses single seeds per PDA.
    if seeds.len() != 1 {
        return Err(format!(
            "Multi-seed PDAs not yet supported (got {} seeds)",
            seeds.len()
        ));
    }

    let seed_bytes: [u8; 32] = match &seeds[0] {
        IdlSeed::Const { value } => {
            // UTF-8 string, right-padded to 32 bytes
            let mut bytes = [0u8; 32];
            let src = value.as_bytes();
            if src.len() > 32 {
                return Err(format!("Const seed '{}' exceeds 32 bytes", value));
            }
            bytes[..src.len()].copy_from_slice(src);
            bytes
        }
        IdlSeed::Account { path } => {
            // Use another account's ID as the seed
            let account_id = account_map
                .get(path)
                .ok_or_else(|| {
                    format!(
                        "PDA seed references account '{}' which hasn't been resolved yet",
                        path
                    )
                })?;
            *account_id.value()
        }
        IdlSeed::Arg { path } => {
            return Err(format!("Arg-based PDA seeds not yet supported (arg: {})", path));
        }
    };

    let pda_seed = PdaSeed::new(seed_bytes);
    Ok(AccountId::from((program_id, &pda_seed)))
}

// ─── Execute ─────────────────────────────────────────────────────

async fn execute_instruction(
    _idl: &NssaIdl,
    ix: &nssa_framework_core::idl::IdlInstruction,
    args: &HashMap<String, String>,
    program_path: &str,
    dry_run: bool,
    treasury_bin: Option<&str>,
    token_bin: Option<&str>,
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
    let mut parsed_args: Vec<(&str, &IdlType, ParsedValue)> = Vec::new();
    let mut has_errors = false;

    for arg in &ix.args {
        let key = snake_to_kebab(&arg.name);
        let raw = args.get(&key).unwrap();
        match parse_value(raw, &arg.type_) {
            Ok(val) => parsed_args.push((&arg.name, &arg.type_, val)),
            Err(e) => {
                eprintln!("❌ --{}: {}", key, e);
                has_errors = true;
            }
        }
    }

    // Parse account IDs for non-PDA accounts
    let mut parsed_accounts: Vec<(&str, Vec<u8>)> = Vec::new();
    for acc in &ix.accounts {
        if acc.pda.is_some() {
            continue;
        }
        let key = format!("{}-account", snake_to_kebab(&acc.name));
        let raw = args.get(&key).unwrap();
        let hex = raw
            .strip_prefix("0x")
            .or_else(|| raw.strip_prefix("0X"))
            .unwrap_or(raw);
        match hex_decode(hex) {
            Ok(bytes) if bytes.len() == 32 => parsed_accounts.push((&acc.name, bytes)),
            Ok(bytes) => {
                eprintln!(
                    "❌ --{}: expected 32 bytes (64 hex chars), got {} bytes",
                    key,
                    bytes.len()
                );
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

    // Build serialized instruction data (risc0 format)
    let ix_index = _idl
        .instructions
        .iter()
        .position(|i| i.name == ix.name)
        .unwrap_or(0);

    let risc0_args: Vec<(&IdlType, &ParsedValue)> = parsed_args
        .iter()
        .map(|(_, ty, val)| (*ty, val))
        .collect();
    let instruction_data = serialize_to_risc0(ix_index as u32, &risc0_args);

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
    for (name, _, val) in &parsed_args {
        println!("  {} = {}", name, val);
    }
    println!();

    println!("🔧 Transaction:");
    println!("  program: {}", program_path);
    println!("  instruction index: {}", ix_index);
    println!("  instruction: {} {{", to_pascal_case(&ix.name));
    for (name, _, val) in &parsed_args {
        println!("    {}: {},", name, val);
    }
    println!("  }}");
    println!();
    println!(
        "  Serialized instruction data ({} u32 words):",
        instruction_data.len()
    );
    let hex_words: Vec<String> = instruction_data.iter().map(|w| format!("{:08x}", w)).collect();
    println!("    [{}]", hex_words.join(", "));
    println!();

    if dry_run {
        println!("⚠️  Dry run — omit --dry-run to submit the transaction.");
        return;
    }

    // ─── Transaction submission ──────────────────────────────────

    let treasury_path = treasury_bin.unwrap_or(program_path);
    let token_path = token_bin.unwrap_or_else(|| {
        eprintln!("❌ --token-bin is required for submission (use --dry-run to skip)");
        process::exit(1);
    });

    println!("📤 Submitting transaction...");

    // Load program binaries to get ProgramIds
    let treasury_bytecode = fs::read(treasury_path).unwrap_or_else(|e| {
        eprintln!("❌ Failed to read treasury binary '{}': {}", treasury_path, e);
        process::exit(1);
    });
    let treasury_program = Program::new(treasury_bytecode).unwrap_or_else(|e| {
        eprintln!("❌ Failed to load treasury program: {:?}", e);
        process::exit(1);
    });
    let treasury_program_id = treasury_program.id();

    let _token_bytecode = fs::read(token_path).unwrap_or_else(|e| {
        eprintln!("❌ Failed to read token binary '{}': {}", token_path, e);
        process::exit(1);
    });
    let _token_program = Program::new(_token_bytecode).unwrap_or_else(|e| {
        eprintln!("❌ Failed to load token program: {:?}", e);
        process::exit(1);
    });

    println!("  Treasury program ID: {:?}", treasury_program_id);

    // Build account map for PDA resolution (non-PDA accounts first)
    let mut account_map: HashMap<String, AccountId> = HashMap::new();
    for (name, bytes) in &parsed_accounts {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(bytes);
        account_map.insert(name.to_string(), AccountId::new(arr));
    }

    // Also store parsed args as ParsedValue for arg-based seeds
    let mut parsed_arg_map: HashMap<String, ParsedValue> = HashMap::new();
    for (name, _, val) in &parsed_args {
        parsed_arg_map.insert(name.to_string(), val.clone());
    }

    // Resolve PDA accounts in instruction order
    for acc in &ix.accounts {
        if let Some(pda) = &acc.pda {
            match compute_pda_from_seeds(
                &pda.seeds,
                &treasury_program_id,
                &account_map,
                &parsed_arg_map,
            ) {
                Ok(id) => {
                    println!("  PDA {} → {}", acc.name, id);
                    account_map.insert(acc.name.clone(), id);
                }
                Err(e) => {
                    eprintln!("❌ Failed to compute PDA for '{}': {}", acc.name, e);
                    process::exit(1);
                }
            }
        }
    }

    // Build account_ids vec in instruction account order
    let mut account_ids: Vec<AccountId> = Vec::new();
    for acc in &ix.accounts {
        let id = account_map.get(&acc.name).unwrap_or_else(|| {
            eprintln!("❌ Account '{}' not resolved", acc.name);
            process::exit(1);
        });
        account_ids.push(*id);
    }

    // Initialize wallet
    let wallet_core = WalletCore::from_env().unwrap_or_else(|e| {
        eprintln!("❌ Failed to initialize wallet: {:?}", e);
        eprintln!("   Set NSSA_WALLET_HOME_DIR environment variable");
        process::exit(1);
    });

    // Collect signer accounts
    let signer_accounts: Vec<AccountId> = ix
        .accounts
        .iter()
        .filter(|a| a.signer)
        .map(|a| *account_map.get(&a.name).unwrap())
        .collect();

    // Fetch nonces and signing keys for signer accounts
    let nonces = if signer_accounts.is_empty() {
        vec![]
    } else {
        wallet_core
            .get_accounts_nonces(signer_accounts.clone())
            .await
            .unwrap_or_else(|e| {
                eprintln!("❌ Failed to fetch nonces: {:?}", e);
                process::exit(1);
            })
    };

    let signing_keys: Vec<_> = signer_accounts
        .iter()
        .map(|id| {
            wallet_core
                .storage()
                .user_data
                .get_pub_account_signing_key(id)
                .unwrap_or_else(|| {
                    eprintln!("❌ Signing key not found for account {}", id);
                    eprintln!("   Was this account created with `wallet account new public`?");
                    process::exit(1);
                })
        })
        .collect();

    // Build and submit transaction
    let message = Message::new_preserialized(
        treasury_program_id,
        account_ids,
        nonces,
        instruction_data,
    );
    let witness_set = WitnessSet::for_message(&message, &signing_keys);
    let tx = PublicTransaction::new(message, witness_set);

    let response = wallet_core
        .sequencer_client
        .send_tx_public(tx)
        .await
        .unwrap_or_else(|e| {
            eprintln!("❌ Failed to submit transaction: {:?}", e);
            process::exit(1);
        });

    println!("📤 Transaction submitted!");
    println!("   tx_hash: {}", response.tx_hash);
    println!("   Waiting for confirmation...");

    let poller = wallet::poller::TxPoller::new(
        wallet_core.config().clone(),
        wallet_core.sequencer_client.clone(),
    );

    match poller.poll_tx(response.tx_hash).await {
        Ok(_) => println!("✅ Transaction confirmed — included in a block."),
        Err(e) => {
            eprintln!("❌ Transaction NOT confirmed: {e:#}");
            eprintln!("   It may have failed execution. Check sequencer logs for details.");
            process::exit(1);
        }
    }
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
            },
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
