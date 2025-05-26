use crate::config::{PingThingsArgs, RpcType};
use crate::tx_senders::{TxResult, TxSender};
use crate::tx_senders::transaction::{build_transaction_with_config, TransactionConfig};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use base64;
use reqwest::Client;
use solana_sdk::{hash::Hash, pubkey::Pubkey, transaction::VersionedTransaction};
use tracing::debug;

pub struct NextBlockTxSender {
    pub name: String,
    pub url: String,
    pub auth_key: String,
    pub args: PingThingsArgs,
    pub client: Client,
}

impl NextBlockTxSender {
    pub fn new(
        name: String,
        url: String,
        auth_key: String,
        args: PingThingsArgs,
        client: Client,
    ) -> Self {
        Self {
            name,
            url,
            auth_key,
            args,
            client,
        }
    }

    fn headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", self.auth_key).parse().unwrap(),
        );
        headers.insert("Content-Type", "application/json".parse().unwrap());
        headers
    }

    fn build_tx(
        &self,
        recent_blockhash: Hash,
        token_address: Pubkey,
        bonding_curve: Pubkey,
        associated_bonding_curve: Pubkey,
    ) -> Result<VersionedTransaction> {
        let tx_config = TransactionConfig::from(self.args.clone());
        let rpc_type = RpcType::Nextblock;

        Ok(build_transaction_with_config(
            &tx_config,
            &rpc_type,
            recent_blockhash,
            token_address,
            bonding_curve,
            associated_bonding_curve,
        ))
    }
}

#[async_trait]
impl TxSender for NextBlockTxSender {
    fn name(&self) -> String {
        self.name.clone()
    }

    async fn send_transaction(
        &self,
        _index: u32,
        recent_blockhash: Hash,
        token_address: Pubkey,
        bonding_curve: Pubkey,
        associated_bonding_curve: Pubkey,
    ) -> Result<TxResult> {
        let tx = self.build_tx(
            recent_blockhash,
            token_address,
            bonding_curve,
            associated_bonding_curve,
        )?;

        let raw_tx = bincode::serialize(&tx)?;
        let encoded = base64::encode(raw_tx);

        let body = serde_json::json!({ "transaction": encoded });

        debug!("Sending tx to NextBlock");

        let resp = self
            .client
            .post(&self.url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            return Err(anyhow!("NextBlock error {}: {}", status, text));
        }

        Ok(TxResult::Signature(tx.signatures[0]))
    }
}
