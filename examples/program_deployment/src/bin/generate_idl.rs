/// Generate IDL JSON for the treasury program.
///
/// The #[nssa_program] macro generates a `__program_idl()` function
/// that returns the full IDL. This binary just calls it and prints JSON.
///
/// Usage:
///   cargo run --bin generate_idl > treasury-idl.json

// We need to replicate the program definition here since the guest binary
// targets riscv32. Instead, we generate the IDL statically from what the
// macro would produce.

use nssa_framework_core::idl::*;

fn main() {
    let idl = NssaIdl {
        version: "0.1.0".to_string(),
        name: "treasury_program".to_string(),
        instructions: vec![
            IdlInstruction {
                name: "create_vault".to_string(),
                accounts: vec![
                    IdlAccountItem {
                        name: "treasury_state_acct".to_string(),
                        writable: true,
                        signer: false,
                        init: true,
                        owner: None,
                        pda: Some(IdlPda {
                            seeds: vec![IdlSeed::Const {
                                value: "treasury_state".to_string(),
                            }],
                        }),
                    },
                    IdlAccountItem {
                        name: "token_definition".to_string(),
                        writable: false,
                        signer: false,
                        init: false,
                        owner: None,
                        pda: None,
                    },
                    IdlAccountItem {
                        name: "vault_holding".to_string(),
                        writable: false,
                        signer: false,
                        init: false,
                        owner: None,
                        pda: Some(IdlPda {
                            seeds: vec![IdlSeed::Account {
                                path: "token_definition".to_string(),
                            }],
                        }),
                    },
                ],
                args: vec![
                    IdlArg {
                        name: "token_name".to_string(),
                        type_: IdlType::Array {
                            array: (Box::new(IdlType::Primitive("u8".to_string())), 6),
                        },
                    },
                    IdlArg {
                        name: "initial_supply".to_string(),
                        type_: IdlType::Primitive("u128".to_string()),
                    },
                    IdlArg {
                        name: "token_program_id".to_string(),
                        type_: IdlType::Primitive("program_id".to_string()),
                    },
                    IdlArg {
                        name: "authorized_accounts".to_string(),
                        type_: IdlType::Vec {
                            vec: Box::new(IdlType::Array {
                                array: (Box::new(IdlType::Primitive("u8".to_string())), 32),
                            }),
                        },
                    },
                ],
            },
            IdlInstruction {
                name: "send".to_string(),
                accounts: vec![
                    IdlAccountItem {
                        name: "treasury_state_acct".to_string(),
                        writable: false,
                        signer: false,
                        init: false,
                        owner: None,
                        pda: Some(IdlPda {
                            seeds: vec![IdlSeed::Const {
                                value: "treasury_state".to_string(),
                            }],
                        }),
                    },
                    IdlAccountItem {
                        name: "vault_holding".to_string(),
                        writable: false,
                        signer: false,
                        init: false,
                        owner: None,
                        pda: Some(IdlPda {
                            seeds: vec![IdlSeed::Account {
                                path: "token_definition".to_string(),
                            }],
                        }),
                    },
                    IdlAccountItem {
                        name: "recipient".to_string(),
                        writable: false,
                        signer: false,
                        init: false,
                        owner: None,
                        pda: None,
                    },
                    IdlAccountItem {
                        name: "signer".to_string(),
                        writable: false,
                        signer: true,
                        init: false,
                        owner: None,
                        pda: None,
                    },
                ],
                args: vec![
                    IdlArg {
                        name: "amount".to_string(),
                        type_: IdlType::Primitive("u128".to_string()),
                    },
                    IdlArg {
                        name: "token_program_id".to_string(),
                        type_: IdlType::Primitive("program_id".to_string()),
                    },
                ],
            },
            IdlInstruction {
                name: "deposit".to_string(),
                accounts: vec![
                    IdlAccountItem {
                        name: "treasury_state_acct".to_string(),
                        writable: false,
                        signer: false,
                        init: false,
                        owner: None,
                        pda: Some(IdlPda {
                            seeds: vec![IdlSeed::Const {
                                value: "treasury_state".to_string(),
                            }],
                        }),
                    },
                    IdlAccountItem {
                        name: "sender_holding".to_string(),
                        writable: false,
                        signer: false,
                        init: false,
                        owner: None,
                        pda: None,
                    },
                    IdlAccountItem {
                        name: "vault_holding".to_string(),
                        writable: false,
                        signer: false,
                        init: false,
                        owner: None,
                        pda: Some(IdlPda {
                            seeds: vec![IdlSeed::Account {
                                path: "token_definition".to_string(),
                            }],
                        }),
                    },
                ],
                args: vec![
                    IdlArg {
                        name: "amount".to_string(),
                        type_: IdlType::Primitive("u128".to_string()),
                    },
                    IdlArg {
                        name: "token_program_id".to_string(),
                        type_: IdlType::Primitive("program_id".to_string()),
                    },
                ],
            },
        ],
        accounts: vec![IdlAccountType {
            name: "TreasuryState".to_string(),
            type_: IdlTypeDef {
                kind: "struct".to_string(),
                fields: vec![
                    IdlField {
                        name: "vault_count".to_string(),
                        type_: IdlType::Primitive("u64".to_string()),
                    },
                    IdlField {
                        name: "authorized_accounts".to_string(),
                        type_: IdlType::Vec {
                            vec: Box::new(IdlType::Array {
                                array: (Box::new(IdlType::Primitive("u8".to_string())), 32),
                            }),
                        },
                    },
                ],
                variants: vec![],
            },
        }],
        types: vec![],
        errors: vec![
            IdlError {
                code: 6001,
                name: "NoAuthorizedAccounts".to_string(),
                msg: Some("At least one authorized account required".to_string()),
            },
            IdlError {
                code: 1008,
                name: "Unauthorized".to_string(),
                msg: Some("Signer is not authorized".to_string()),
            },
        ],
    };

    println!("{}", idl.to_json_pretty().unwrap());
}
