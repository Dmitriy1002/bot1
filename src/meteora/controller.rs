use crate::config::PingThingsArgs;
use crate::core::extract_instructions;
use crate::meteora::fetch_and_swap::fetch_and_execute_swap;
use crate::metrics::{METEORA_POOL_DETECTED, METEORA_SWAP_FAILURE, METEORA_SWAP_SUCCESS};
use crate::tx_senders::constants::{METEORA_PROGRAM_ID, WSOL_MINT};

use anyhow::Result;
use solana_sdk::{signature::Keypair, transaction::VersionedTransaction};
use std::sync::Arc;
use tracing::{info, warn};
use yellowstone_grpc_proto::prelude::TransactionStatusMeta as YellowstoneMeta;
use yellowstone_grpc_proto::convert_from::create_tx_meta;
use solana_transaction_status::TransactionStatusMeta;

#[derive(Clone)]
pub struct MeteoraController {
    pub args: PingThingsArgs,
    pub user: Arc<Keypair>,
}

impl MeteoraController {
    pub fn new(args: PingThingsArgs, user: Arc<Keypair>) -> Self {
        Self { args, user }
    }

pub async fn transaction_handler(
    &self,
    tx: VersionedTransaction,
    meta: YellowstoneMeta,
) -> Result<()> {
    info!("Обработка новой транзакции");

    if meta.err.is_some() {
        info!("Пропущена failed-транзакция");
        return Ok(());
    }

    let parsed_meta: TransactionStatusMeta = match create_tx_meta(meta.clone()) {
        Ok(m) => m,
        Err(e) => {
            warn!("Не удалось сконвертировать meta: {:?}", e);
            return Ok(());
        }
    };

    let Ok(inner_ixs) = extract_instructions(parsed_meta, tx.clone()) else {
        warn!("Не удалось извлечь инструкции из транзакции");
        return Ok(());
    };

    info!("Найдено {} инструкций", inner_ixs.len());

    for ix in inner_ixs {
        if ix.program_id != METEORA_PROGRAM_ID {
            continue;
        }

        info!("Инструкция от METEORA_PROGRAM_ID");

        let data = &ix.data;
        let discriminator = &data[..8];
        let expected: [u8; 8] = [132, 245, 78, 173, 170, 147, 105, 179]; // initPool

        if discriminator != expected {
            info!("Дискриминатор не совпадает с initPool: {:?}", discriminator);
            continue;
        }

        if ix.accounts.len() <= 6 {
            warn!("Недостаточно аккаунтов в инструкции");
            continue;
        }

        let pool_account = &ix.accounts[1];
        let token_a = &ix.accounts[5];
        let token_b = &ix.accounts[6];

        info!(
            "Обнаружен новый пул: {:?} | Token A: {:?} | Token B: {:?}",
            pool_account.pubkey, token_a.pubkey, token_b.pubkey
        );
        METEORA_POOL_DETECTED.inc();

        if token_a.pubkey.to_string() == WSOL_MINT || token_b.pubkey.to_string() == WSOL_MINT {
            info!("Один из токенов — WSOL, пробуем выполнить swap");

            let result = fetch_and_execute_swap(
                &self.args,
                &self.user,
                &pool_account.pubkey,
                &token_a.pubkey,
                &token_b.pubkey,
            )
            .await;

            match result {
                Ok(_) => {
                    info!("Swap выполнен успешно");
                    METEORA_SWAP_SUCCESS.inc();
                }
                Err(e) => {
                    warn!("Ошибка при swap: {:?}", e);
                    METEORA_SWAP_FAILURE.inc();
                }
            }
        } else {
            info!("Пропуск: ни один токен не является WSOL");
        }
    }

    Ok(())
}

}