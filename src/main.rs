mod config;
mod geyser;
mod core;
mod tx_senders;
mod meteora;
mod metrics;
mod metrics_server;

use crate::config::PingThingsArgs;
use crate::geyser::{YellowstoneGrpcGeyser, YellowstoneGrpcGeyserClient};
use crate::meteora::controller::MeteoraController;
use crate::metrics_server::start_metrics_server;
use crate::tx_senders::constants::METEORA_PROGRAM_ID;

use solana_sdk::signature::Keypair;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc};
use tracing::{info, warn};
use tokio::sync::RwLock;
use tracing_subscriber::FmtSubscriber;
use yellowstone_grpc_proto::geyser::{CommitmentLevel, SubscribeRequestFilterTransactions};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Инициализация логгера
    FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Загрузка конфигурации
    let args = PingThingsArgs::new();
    let user = Arc::new(Keypair::from_base58_string(&args.private_key));
    let meteora_controller = MeteoraController::new(args.clone(), user.clone());

    // Запуск HTTP-сервера для метрик Prometheus
    tokio::spawn(async {
        start_metrics_server().await;
    });

    // Настройка фильтра транзакций для Meteora
    let mut transaction_filters = HashMap::new();
    let meteora_filter = SubscribeRequestFilterTransactions {
        vote: Some(false),
        failed: Some(false),
        account_include: vec![METEORA_PROGRAM_ID.to_string()],
        account_exclude: vec![],
        account_required: vec![],
        signature: None,
    };
    transaction_filters.insert("meteora_transaction_filter".to_string(), meteora_filter);

    let geyser = YellowstoneGrpcGeyserClient::new(
        args.geyser_url.clone(),
        Some(args.geyser_x_token.clone()),
        Some(CommitmentLevel::Processed),
        HashMap::new(),
        transaction_filters,
        Arc::new(RwLock::new(HashSet::new())),
    );

    info!("Подключение к Geyser установлено");

    // Подписка на поток транзакций через Geyser
    geyser
        .consume(move |tx, meta| {
            let meteora_controller = meteora_controller.clone();
            Box::pin(async move {
                let _ = meteora_controller.transaction_handler(tx, meta).await;
            })
        })
        .await?;

    Ok(())
}