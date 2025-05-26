use crate::config::PingThingsArgs;
use crate::meteora::fetch_and_swap::fetch_and_execute_swap;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::hash::Hash;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use std::sync::Arc;
use tokio::time::Instant;
use tracing::{error, info};

#[derive(Clone)]
pub struct Bench {
    config: PingThingsArgs,
    user: Arc<Keypair>,
    pool: Pubkey,
    token_a: Pubkey,
    token_b: Pubkey,
    a_vault: Pubkey,
    b_vault: Pubkey,
    vault_token_a: Pubkey,
    vault_token_b: Pubkey,
    lp_mint_a: Pubkey,
    lp_mint_b: Pubkey,
    a_lp: Pubkey,
    b_lp: Pubkey,
    protocol_fee: Pubkey,
    vault_program: Pubkey,
}

impl Bench {
    pub fn new(
        config: PingThingsArgs,
        pool: Pubkey,
        token_a: Pubkey,
        token_b: Pubkey,
        a_vault: Pubkey,
        b_vault: Pubkey,
        vault_token_a: Pubkey,
        vault_token_b: Pubkey,
        lp_mint_a: Pubkey,
        lp_mint_b: Pubkey,
        a_lp: Pubkey,
        b_lp: Pubkey,
        protocol_fee: Pubkey,
        vault_program: Pubkey,
    ) -> Self {
        let user = Arc::new(Keypair::from_base58_string(&config.private_key));

        Self {
            config,
            user,
            pool,
            token_a,
            token_b,
            a_vault,
            b_vault,
            vault_token_a,
            vault_token_b,
            lp_mint_a,
            lp_mint_b,
            a_lp,
            b_lp,
            protocol_fee,
            vault_program,
        }
    }

    pub async fn run(self, repeats: usize) {
        let mut handles = vec![];
        let start = Instant::now();

        for i in 0..repeats {
            let bench = self.clone();
            let user = bench.user.clone();

            let handle = tokio::spawn(async move {
                let result = fetch_and_execute_swap(
                    &bench.config,
                    &user,
                    &bench.pool,
                    &bench.token_a,
                    &bench.token_b,
                )
                .await;

                match result {
                    Ok(_) => info!("Swap {} complete", i),
                    Err(e) => error!("Swap {} failed: {:?}", i, e),
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.await;
        }

        let elapsed = start.elapsed().as_millis();
        info!("🏁 All {} swaps completed in {} ms", repeats, elapsed);
    }
}