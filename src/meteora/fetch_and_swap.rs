use crate::meteora::fetch_pool::fetch_pool_accounts;
use crate::config::PingThingsArgs;
use crate::tx_senders::constants::VAULT_PROGRAM_ID;
use crate::tx_senders::transaction::build_swap_transaction;

use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use tracing::{info, warn, debug};

pub async fn fetch_and_execute_swap(
    args: &PingThingsArgs,
    user: &Keypair,
    pool_account: &Pubkey,
    token_a: &Pubkey,
    token_b: &Pubkey,
) -> Result<()> {
    info!("Запуск свапа через Meteora");
    debug!("Адрес пула: {pool_account}, Token A: {token_a}, Token B: {token_b}");

    let rpc = RpcClient::new(args.http_rpc.clone());

    info!("Получение информации о пуле...");
    let pool_info = match fetch_pool_accounts(&rpc, pool_account).await {
        Ok(info) => {
            info!("Информация о пуле получена");
            info
        }
        Err(e) => {
            warn!("Не удалось получить информацию о пуле: {:?}", e);
            return Err(e);
        }
    };

    info!("Сборка транзакции swap...");
    let tx = match build_swap_transaction(
        args,
        user,
        pool_account,
        token_a,
        token_b,
        &pool_info.a_vault,
        &pool_info.b_vault,
        &pool_info.token_a_mint,
        &pool_info.token_b_mint,
        &pool_info.lp_mint,
        &pool_info.lp_mint,
        &pool_info.a_vault_lp,
        &pool_info.b_vault_lp,
        &pool_info.protocol_token_a_fee,
        &VAULT_PROGRAM_ID,
        100_000_000,
        1,
    )
    .await {
        Ok(tx) => {
            info!("Транзакция успешно собрана");
            tx
        }
        Err(e) => {
            warn!("Ошибка сборки транзакции: {:?}", e);
            return Err(e);
        }
    };

    info!("Отправка транзакции...");
    match rpc.send_and_confirm_transaction(&tx).await {
        Ok(sig) => {
            info!("Транзакция успешно отправлена! Signature: {}", sig);
            Ok(())
        }
        Err(e) => {
            warn!("Ошибка сборки транзакции: {:?}", e);
            return Err(e.into());
        }
    }
}
