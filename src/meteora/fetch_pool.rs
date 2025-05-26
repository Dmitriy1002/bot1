use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use anyhow::Result;
use borsh::{BorshDeserialize};

#[derive(BorshDeserialize, Debug)]
pub struct PoolAccountData {
    pub lp_mint: Pubkey,
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub a_vault: Pubkey,
    pub b_vault: Pubkey,
    pub a_vault_lp: Pubkey,
    pub b_vault_lp: Pubkey,
    pub a_vault_lp_bump: u8,
    pub enabled: bool,
    pub protocol_token_a_fee: Pubkey,
    pub protocol_token_b_fee: Pubkey,
}

pub async fn fetch_pool_accounts(rpc: &RpcClient, pool_pubkey: &Pubkey) -> Result<PoolAccountData> {
    let account = rpc.get_account(pool_pubkey).await?;
    let data = PoolAccountData::try_from_slice(&account.data)?;
    Ok(data)
}