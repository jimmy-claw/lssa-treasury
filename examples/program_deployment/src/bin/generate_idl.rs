/// Generate IDL JSON for the treasury program.
///
/// This reads the program source and extracts the IDL from
/// #[nssa_program] annotations — no manual IDL definition needed.
///
/// Usage:
///   cargo run --bin generate_idl > treasury-idl.json

nssa_framework::generate_idl!("../../methods/guest/src/bin/treasury.rs");
