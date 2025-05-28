use crate::config::PingThingsArgs;
use crate::core::extract_instructions;
use crate::meteora::fetch_and_swap::fetch_and_execute_swap;
use crate::metrics::{METEORA_POOL_DETECTED, METEORA_SWAP_FAILURE, METEORA_SWAP_SUCCESS};
use crate::tx_senders::constants::{METEORA_PROGRAM_ID, WSOL_MINT};

use anyhow::Result;
use solana_sdk::{signature::Keypair, transaction::VersionedTransaction};
use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use tracing::{info, warn};
use yellowstone_grpc_proto::prelude::TransactionStatusMeta as YellowstoneMeta;
use yellowstone_grpc_proto::convert_from::create_tx_meta;
use solana_transaction_status::TransactionStatusMeta;

#[derive(Clone)]
pub struct MeteoraController {
    pub args: PingThingsArgs,
    pub user: Arc<Keypair>,
    pub seen_pools: Arc<RwLock<HashSet<String>>>,
}

impl MeteoraController {
    pub fn new(args: PingThingsArgs, user: Arc<Keypair>) -> Self {
        Self {
            args,
            user,
            seen_pools: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    pub async fn transaction_handler(
        &self,
        tx: VersionedTransaction,
        meta: YellowstoneMeta,
    ) -> Result<()> {
        if meta.err.is_some() {
            return Ok(()); // Пропускаем failed-транзакции
        }

        let parsed_meta: TransactionStatusMeta = match create_tx_meta(meta.clone()) {
            Ok(m) => m,
            Err(e) => {
                warn!("Не удалось сконвертировать meta: {:?}", e);
                return Ok(());
            }
        };

        let Ok(inner_ixs) = extract_instructions(parsed_meta, tx.clone()) else {
            warn!("Не удалось извлечь инструкции");
            return Ok(());
        };

        for ix in inner_ixs {
            if ix.program_id != METEORA_PROGRAM_ID {
                continue;
            }

            if ix.accounts.len() <= 6 {
                continue;
            }

            let pool_account = &ix.accounts[1];
            let token_a = &ix.accounts[5];
            let token_b = &ix.accounts[6];

            let pool_key = pool_account.pubkey.to_string();

            {
                let seen = self.seen_pools.read().unwrap();
                if seen.contains(&pool_key) {
                    continue;
                }
            }

            self.seen_pools.write().unwrap().insert(pool_key.clone());

            info!("Обнаружен новый пул: {} | Token A: {} | Token B: {}",
                pool_key, token_a.pubkey, token_b.pubkey);
            METEORA_POOL_DETECTED.inc();

            if token_a.pubkey.to_string() == WSOL_MINT || token_b.pubkey.to_string() == WSOL_MINT {
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
                        info!("Swap выполнен для пула: {}", pool_key);
                        METEORA_SWAP_SUCCESS.inc();
                    }
                    Err(e) => {
                        warn!("Ошибка swap: {:?}", e);
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
