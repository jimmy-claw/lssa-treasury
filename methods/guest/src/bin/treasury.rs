#![no_main]

// The treasury program using nssa-framework macros.
//
// Previously this file was ~20 lines of boilerplate:
//   read inputs → match instruction → dispatch → write outputs
//
// Now the #[nssa_program] macro generates all of that from the
// function signatures below. The Instruction enum is auto-generated.

use nssa_core::account::{Account, AccountId, AccountWithMetadata};
use nssa_core::program::{AccountPostState, ChainedCall, ProgramId};
use nssa_framework::prelude::*;
use treasury_core::{TreasuryState, vault_holding_pda_seed};

risc0_zkvm::guest::entry!(main);

#[nssa_program]
mod treasury_program {
    #[allow(unused_imports)]
    use super::*;

    /// Create a new token vault with initial supply.
    /// Accounts: [treasury_state, token_definition, vault_holding]
    #[instruction]
    pub fn create_vault(
        treasury_state_acct: AccountWithMetadata,
        token_definition: AccountWithMetadata,
        vault_holding: AccountWithMetadata,
        token_name: [u8; 6],
        initial_supply: u128,
        token_program_id: ProgramId,
        authorized_accounts: Vec<[u8; 32]>,
    ) -> NssaResult {
        if authorized_accounts.is_empty() {
            return Err(NssaError::custom(1, "At least one authorized account required"));
        }

        let is_first_time = treasury_state_acct.account == Account::default();
        let mut state: TreasuryState = if is_first_time {
            TreasuryState::default()
        } else {
            let data: Vec<u8> = treasury_state_acct.account.data.clone().into();
            borsh::from_slice(&data).map_err(|e| NssaError::DeserializationError {
                account_index: 0,
                message: e.to_string(),
            })?
        };
        state.vault_count += 1;
        state.authorized_accounts = authorized_accounts;

        let mut treasury_post = treasury_state_acct.account.clone();
        let state_bytes = borsh::to_vec(&state).map_err(|e| NssaError::SerializationError {
            message: e.to_string(),
        })?;
        treasury_post.data = state_bytes.try_into().expect("TreasuryState too large");

        let treasury_post_state = if is_first_time {
            AccountPostState::new_claimed(treasury_post)
        } else {
            AccountPostState::new(treasury_post)
        };

        // Build chained call to Token::NewFungibleDefinition
        let mut token_ix_bytes = vec![0u8; 23];
        token_ix_bytes[0] = 0x00;
        token_ix_bytes[1..17].copy_from_slice(&initial_supply.to_le_bytes());
        token_ix_bytes[17..23].copy_from_slice(&token_name);
        let instruction_data = risc0_zkvm::serde::to_vec(&token_ix_bytes).unwrap();

        let mut vault_holding_authorized = vault_holding.clone();
        vault_holding_authorized.is_authorized = true;

        let chained_call = ChainedCall {
            program_id: token_program_id,
            instruction_data,
            pre_states: vec![token_definition.clone(), vault_holding_authorized],
            pda_seeds: vec![vault_holding_pda_seed(&token_definition.account_id)],
        };

        Ok(NssaOutput::with_chained_calls(
            vec![
                treasury_post_state,
                AccountPostState::new(token_definition.account.clone()),
                AccountPostState::new(vault_holding.account.clone()),
            ],
            vec![chained_call],
        ))
    }

    /// Send tokens from the treasury vault to a recipient.
    /// Accounts: [treasury_state, vault_holding, recipient, signer]
    #[instruction]
    pub fn send(
        treasury_state_acct: AccountWithMetadata,
        vault_holding: AccountWithMetadata,
        recipient: AccountWithMetadata,
        signer: AccountWithMetadata,
        amount: u128,
        token_program_id: ProgramId,
    ) -> NssaResult {
        let state_data: Vec<u8> = treasury_state_acct.account.data.clone().into();
        let state: TreasuryState = borsh::from_slice(&state_data)
            .map_err(|e| NssaError::DeserializationError {
                account_index: 0,
                message: e.to_string(),
            })?;

        let signer_bytes = *signer.account_id.value();
        if !state.authorized_accounts.iter().any(|a| *a == signer_bytes) {
            return Err(NssaError::Unauthorized {
                message: "Signer is not an authorized account".into(),
            });
        }
        if !signer.is_authorized {
            return Err(NssaError::Unauthorized {
                message: "Transaction not signed by this key".into(),
            });
        }

        // Extract definition_id from vault holding data
        let vault_data: Vec<u8> = vault_holding.account.data.clone().into();
        if vault_data.len() < 33 {
            return Err(NssaError::DeserializationError {
                account_index: 1,
                message: format!("vault data too short (len={})", vault_data.len()),
            });
        }
        let mut def_id_bytes = [0u8; 32];
        def_id_bytes.copy_from_slice(&vault_data[1..33]);
        let definition_id = AccountId::new(def_id_bytes);

        let mut token_ix_bytes = vec![0u8; 23];
        token_ix_bytes[0] = 0x01;
        token_ix_bytes[1..17].copy_from_slice(&amount.to_le_bytes());
        let instruction_data = risc0_zkvm::serde::to_vec(&token_ix_bytes).unwrap();

        let mut vault_authorized = vault_holding.clone();
        vault_authorized.is_authorized = true;

        let chained_call = ChainedCall {
            program_id: token_program_id,
            instruction_data,
            pre_states: vec![vault_authorized, recipient.clone()],
            pda_seeds: vec![vault_holding_pda_seed(&definition_id)],
        };

        Ok(NssaOutput::with_chained_calls(
            vec![
                AccountPostState::new(treasury_state_acct.account.clone()),
                AccountPostState::new(vault_holding.account.clone()),
                AccountPostState::new(recipient.account.clone()),
                AccountPostState::new(signer.account.clone()),
            ],
            vec![chained_call],
        ))
    }

    /// Deposit tokens into the vault from an external sender.
    /// Accounts: [treasury_state, sender_holding, vault_holding]
    #[instruction]
    pub fn deposit(
        treasury_state_acct: AccountWithMetadata,
        sender_holding: AccountWithMetadata,
        vault_holding: AccountWithMetadata,
        amount: u128,
        token_program_id: ProgramId,
    ) -> NssaResult {
        let mut token_ix_bytes = vec![0u8; 23];
        token_ix_bytes[0] = 0x01;
        token_ix_bytes[1..17].copy_from_slice(&amount.to_le_bytes());
        let instruction_data = risc0_zkvm::serde::to_vec(&token_ix_bytes).unwrap();

        let chained_call = ChainedCall {
            program_id: token_program_id,
            instruction_data,
            pre_states: vec![sender_holding.clone(), vault_holding.clone()],
            pda_seeds: vec![],
        };

        Ok(NssaOutput::with_chained_calls(
            vec![
                AccountPostState::new(treasury_state_acct.account.clone()),
                AccountPostState::new(sender_holding.account.clone()),
                AccountPostState::new(vault_holding.account.clone()),
            ],
            vec![chained_call],
        ))
    }
}
