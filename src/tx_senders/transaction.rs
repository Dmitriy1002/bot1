use crate::config::PingThingsArgs;
use crate::config::RpcType;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    hash::Hash,
    instruction::{AccountMeta, Instruction},
    message::{v0::Message, VersionedMessage},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::VersionedTransaction,
};
use spl_associated_token_account::{get_associated_token_address, instruction::create_associated_token_account};
use solana_sdk::system_program;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use anyhow::Result;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{info, debug, warn};

use crate::tx_senders::constants::{
    JITO_TIP_ADDR, PUMP_FUN_ACCOUNT_ADDR, PUMP_FUN_PROGRAM_ADDR, PUMP_FUN_TX_ADDR,
    RENT_ADDR, SYSTEM_PROGRAM_ADDR, TOKEN_PROGRAM_ADDR,
};

#[derive(Clone)]
pub struct TransactionConfig {
    pub keypair: Arc<Keypair>,
    pub compute_unit_limit: u32,
    pub compute_unit_price: u64,
    pub tip: u64,
    pub buy_amount: u64,
    pub min_amount_out: u64,
}

impl From<PingThingsArgs> for TransactionConfig {
    fn from(args: PingThingsArgs) -> Self {
        let keypair = Keypair::from_base58_string(args.private_key.as_str());

        TransactionConfig {
            keypair: Arc::new(keypair),
            compute_unit_limit: args.compute_unit_limit,
            compute_unit_price: args.compute_unit_price,
            tip: (args.tip * LAMPORTS_PER_SOL as f64) as u64,
            buy_amount: (args.buy_amount * LAMPORTS_PER_SOL as f64) as u64,
            min_amount_out: (args.min_amount_out * 1_000_000.0) as u64,
        }
    }
}

// Pump.fun
pub fn build_transaction_with_config(
    tx_config: &TransactionConfig,
    rpc_type: &RpcType,
    recent_blockhash: Hash,
    token_address: Pubkey,
    bonding_curve: Pubkey,
    associated_bonding_curve: Pubkey,
) -> VersionedTransaction {
    info!("Сборка транзакции Pump.fun");
    let mut instructions = Vec::new();

    if tx_config.compute_unit_limit > 0 {
        instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(tx_config.compute_unit_limit));
        debug!("compute_unit_limit: {}", tx_config.compute_unit_limit);
    }

    if tx_config.compute_unit_price > 0 {
        instructions.push(ComputeBudgetInstruction::set_compute_unit_price(tx_config.compute_unit_price));
        debug!("compute_unit_price: {}", tx_config.compute_unit_price);
    }

    if tx_config.tip > 0 {
        if let RpcType::Jito = rpc_type {
            debug!("Добавление чаевых Jito: {} лампортов", tx_config.tip);
            instructions.push(system_instruction::transfer(
                &tx_config.keypair.pubkey(),
                &Pubkey::from_str(JITO_TIP_ADDR).unwrap(),
                tx_config.tip,
            ));
        }
    }

    let owner = tx_config.keypair.pubkey();
    let token_program_pubkey = Pubkey::from_str(TOKEN_PROGRAM_ADDR).unwrap();
    let user_token_account = get_associated_token_address(&owner, &token_address);
    let ata_ix = create_associated_token_account(&owner, &owner, &token_address, &token_program_pubkey);
    instructions.push(ata_ix);

    let mut data = vec![];
    let buy: u64 = 16927863322537952870;
    data.extend_from_slice(&buy.to_le_bytes());
    data.extend_from_slice(&tx_config.min_amount_out.to_le_bytes());
    data.extend_from_slice(&tx_config.buy_amount.to_le_bytes());

    debug!("Подготовка инструкции swap");

    let accounts = vec![
        AccountMeta::new_readonly(Pubkey::from_str(PUMP_FUN_ACCOUNT_ADDR).unwrap(), false),
        AccountMeta::new(Pubkey::from_str("CebN5WGQ4jvEPvsVU4EoHEpgzq1VV7AbicfhtW4xC9iM").unwrap(), false),
        AccountMeta::new_readonly(token_address, false),
        AccountMeta::new(bonding_curve, false),
        AccountMeta::new(associated_bonding_curve, false),
        AccountMeta::new(user_token_account, false),
        AccountMeta::new(owner, true),
        AccountMeta::new_readonly(Pubkey::from_str(SYSTEM_PROGRAM_ADDR).unwrap(), false),
        AccountMeta::new_readonly(token_program_pubkey, false),
        AccountMeta::new_readonly(Pubkey::from_str(RENT_ADDR).unwrap(), false),
        AccountMeta::new_readonly(Pubkey::from_str(PUMP_FUN_TX_ADDR).unwrap(), false),
        AccountMeta::new_readonly(Pubkey::from_str(PUMP_FUN_PROGRAM_ADDR).unwrap(), false),
    ];

    let swap_instruction = Instruction {
        program_id: Pubkey::from_str(PUMP_FUN_PROGRAM_ADDR).unwrap(),
        accounts,
        data,
    };

    instructions.push(swap_instruction);

    let message_v0 = Message::try_compile(&owner, &instructions, &[], recent_blockhash).unwrap();
    let versioned_message = VersionedMessage::V0(message_v0);
    let tx = VersionedTransaction::try_new(versioned_message, &[&tx_config.keypair]).unwrap();

    info!("Транзакция Pump.fun успешно собрана");

    tx
}

// Meteora
pub async fn build_swap_transaction(
    args: &PingThingsArgs,
    user: &Keypair,
    pool: &Pubkey,
    token_a: &Pubkey,
    token_b: &Pubkey,
    a_vault: &Pubkey,
    b_vault: &Pubkey,
    vault_token_a: &Pubkey,
    vault_token_b: &Pubkey,
    lp_mint_a: &Pubkey,
    lp_mint_b: &Pubkey,
    a_lp: &Pubkey,
    b_lp: &Pubkey,
    protocol_fee: &Pubkey,
    vault_program: &Pubkey,
    amount_in: u64,
    min_out: u64,
) -> Result<VersionedTransaction> {
    info!("Сборка транзакции Meteora");
    let mut instructions: Vec<Instruction> = Vec::new();
    let owner = user.pubkey();

    if args.compute_unit_limit > 0 {
        instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(args.compute_unit_limit));
        debug!("compute_unit_limit: {}", args.compute_unit_limit);
    }

    if args.compute_unit_price > 0 {
        instructions.push(ComputeBudgetInstruction::set_compute_unit_price(args.compute_unit_price));
        debug!("compute_unit_price: {}", args.compute_unit_price);
    }

    let user_token_account = get_associated_token_address(&owner, token_a);
    let token_program_id = Pubkey::from_str(TOKEN_PROGRAM_ADDR).unwrap();
    let ata_ix = create_associated_token_account(&owner, &owner, token_a, &token_program_id);
    instructions.push(ata_ix);

    let mut data = vec![1]; // swap discriminator
    data.extend_from_slice(&amount_in.to_le_bytes());
    data.extend_from_slice(&min_out.to_le_bytes());

    debug!("Подготовка инструкции swap Meteora");

    let accounts = vec![
        AccountMeta::new_readonly(*pool, false),
        AccountMeta::new(*a_vault, false),
        AccountMeta::new(*b_vault, false),
        AccountMeta::new(*vault_token_a, false),
        AccountMeta::new(*vault_token_b, false),
        AccountMeta::new(*a_lp, false),
        AccountMeta::new(*b_lp, false),
        AccountMeta::new_readonly(*protocol_fee, false),
        AccountMeta::new(user_token_account, false),
        AccountMeta::new(owner, true),
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    let swap_instruction = Instruction {
        program_id: *vault_program,
        accounts,
        data,
    };

    instructions.push(swap_instruction);

    let rpc = RpcClient::new(args.http_rpc.clone());
    info!("Получение blockhash...");
    let blockhash = rpc.get_latest_blockhash().await?;
    debug!("Blockhash: {:?}", blockhash);

    let message = Message::try_compile(&owner, &instructions, &[], blockhash)?;
    let versioned_message = VersionedMessage::V0(message);

    let transaction = VersionedTransaction::try_new(versioned_message, &[user])?;
    info!("Транзакция Meteora успешно собрана");

    Ok(transaction)
}