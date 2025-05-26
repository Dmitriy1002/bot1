use async_trait::async_trait;
use futures::StreamExt;
use solana_sdk::{pubkey::Pubkey, transaction::VersionedTransaction};
use yellowstone_grpc_proto::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use yellowstone_grpc_client::GeyserGrpcClient;
use yellowstone_grpc_proto::tonic::transport::ClientTlsConfig;
use yellowstone_grpc_proto::geyser::{
    subscribe_update::UpdateOneof, CommitmentLevel, SubscribeRequest,
    SubscribeRequestFilterAccounts, SubscribeRequestFilterTransactions,
};
use thiserror::Error;
use futures::future::BoxFuture;
use yellowstone_grpc_proto::convert_from::create_tx_versioned;

#[derive(Debug)]
pub struct YellowstoneGrpcGeyserClient {
    pub endpoint: String,
    pub x_token: Option<String>,
    pub commitment: Option<CommitmentLevel>,
    pub account_filters: HashMap<String, SubscribeRequestFilterAccounts>,
    pub transaction_filters: HashMap<String, SubscribeRequestFilterTransactions>,
    pub account_deletions_tracked: Arc<RwLock<HashSet<Pubkey>>>,
}

impl YellowstoneGrpcGeyserClient {
    pub fn new(
        endpoint: String,
        x_token: Option<String>,
        commitment: Option<CommitmentLevel>,
        account_filters: HashMap<String, SubscribeRequestFilterAccounts>,
        transaction_filters: HashMap<String, SubscribeRequestFilterTransactions>,
        account_deletions_tracked: Arc<RwLock<HashSet<Pubkey>>>,
    ) -> Self {
        Self {
            endpoint,
            x_token,
            commitment,
            account_filters,
            transaction_filters,
            account_deletions_tracked,
        }
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Custom error: {0}")]
    Custom(String),
}

pub type GeyserResult<T> = Result<T, Error>;

#[async_trait]
pub trait YellowstoneGrpcGeyser: Send + Sync {
    async fn consume<F>(&self, handler: F) -> GeyserResult<()>
    where
        F: Fn(VersionedTransaction, TransactionStatusMeta) -> BoxFuture<'static, ()>
            + Send
            + Sync
            + 'static;
}

#[async_trait]
impl YellowstoneGrpcGeyser for YellowstoneGrpcGeyserClient {
    async fn consume<F>(&self, handler: F) -> GeyserResult<()>
    where
        F: Fn(VersionedTransaction, TransactionStatusMeta) -> BoxFuture<'static, ()>
            + Send
            + Sync
            + 'static,
    {
        let mut geyser_client = GeyserGrpcClient::build_from_shared(self.endpoint.clone())
            .map_err(|err| Error::Custom(err.to_string()))?
            .x_token(self.x_token.clone())
            .map_err(|err| Error::Custom(err.to_string()))?
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(15))
            .tls_config(ClientTlsConfig::new().with_enabled_roots())
            .map_err(|err| Error::Custom(err.to_string()))?
            .connect()
            .await
            .map_err(|err| Error::Custom(err.to_string()))?;

        let subscribe_request = SubscribeRequest {
            slots: HashMap::new(),
            accounts: self.account_filters.clone(),
            transactions: self.transaction_filters.clone(),
            transactions_status: HashMap::new(),
            entry: HashMap::new(),
            blocks: HashMap::new(),
            blocks_meta: HashMap::new(),
            commitment: self.commitment.map(|x| x as i32),
            accounts_data_slice: vec![],
            ping: None,
        };

        let (mut _subscribe_tx, mut stream) =
            geyser_client.subscribe_with_request(Some(subscribe_request)).await
                .map_err(|err| Error::Custom(err.to_string()))?;

        while let Some(message) = stream.next().await {
            match message {
                Ok(msg) => {
                    if let Some(UpdateOneof::Transaction(tx_update)) = msg.update_oneof {
                        if let Some(tx_info) = tx_update.transaction {
                            let Some(raw_tx) = tx_info.transaction else {
                                log::warn!("Нет поля transaction");
                                continue;
                            };
                            let Some(meta) = tx_info.meta else {
                                log::warn!("Нет поля meta");
                                continue;
                            };

                            let Ok(versioned_tx) = create_tx_versioned(raw_tx) else {
                                log::warn!("Не удалось сконвертировать transaction");
                                continue;
                            };

                            handler(versioned_tx, meta).await;
                        }
                    }
                }
                Err(error) => {
                    log::error!("Geyser stream error: {:?}", error);
                    break;
                }
            }
        }

        Ok(())
    }
}