pub mod constants;
pub mod transaction;
pub mod bloxroute;
pub mod nextblock;

use solana_sdk::{hash::Hash, pubkey::Pubkey, signature::Signature};
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub enum TxResult {
    Signature(Signature),
}

impl Into<String> for TxResult {
    fn into(self) -> String {
        match self {
            TxResult::Signature(sig) => sig.to_string(),
        }
    }
}

#[async_trait]
pub trait TxSender: Sync + Send {
    fn name(&self) -> String;

    async fn send_transaction(
        &self,
        index: u32,
        recent_blockhash: Hash,
        token_address: Pubkey,
        bonding_curve: Pubkey,
        associated_bonding_curve: Pubkey,
    ) -> anyhow::Result<TxResult>;
}