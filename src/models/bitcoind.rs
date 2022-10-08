use bitcoin::blockdata::block::Block;
use bitcoin::blockdata::transaction::Transaction;
use bitcoin::hash_types::BlockHash;
use bitcoin::util::address::Address;
use bitcoin::util::uint::Uint256;
use bitcoin::Amount;
use bitcoincore_rpc::bitcoincore_rpc_json::{EstimateMode, FundRawTransactionOptions};
use bitcoincore_rpc::Client;
use bitcoincore_rpc::RpcApi;
use hex;
use lightning::chain::chaininterface::{BroadcasterInterface, ConfirmationTarget, FeeEstimator};
use lightning_block_sync::{
    AsyncBlockSourceResult, BlockHeaderData, BlockSource, BlockSourceError,
};
use std::collections::HashMap;
use std::sync::Arc;

pub struct FundedTx {
    pub changepos: i64,
    pub hex: String,
}

pub struct SignedTx {
    pub complete: bool,
    pub hex: String,
}

#[derive(Clone)]
pub struct LdkBitcoindClient {
    pub bitcoind_client: Arc<Client>,
}

impl LdkBitcoindClient {
    pub fn create_raw_transaction(&self, outputs: HashMap<String, Amount>) -> String {
        self.bitcoind_client
            .create_raw_transaction_hex(&vec![], &outputs, None, None)
            .unwrap()
    }

    pub fn fund_raw_transaction(&self, raw_tx: String) -> FundedTx {
        let options = FundRawTransactionOptions {
            fee_rate: Some(
                Amount::from_sat(
                    self.get_est_sat_per_1000_weight(ConfirmationTarget::Normal) as u64
                ), // used to divide by 250.0??
            ),
            replaceable: Some(false),
            ..Default::default()
        };

        let funded_tx = self
            .bitcoind_client
            .fund_raw_transaction(raw_tx, Some(&options), None)
            .unwrap();

        FundedTx {
            changepos: funded_tx.change_position as i64,
            hex: hex::encode(funded_tx.hex),
        }
    }

    pub fn sign_raw_transaction_with_wallet(&self, tx_hex: String) -> SignedTx {
        let signed_tx = self
            .bitcoind_client
            .sign_raw_transaction_with_wallet(tx_hex, None, None)
            .unwrap();

        SignedTx {
            complete: signed_tx.complete,
            hex: hex::encode(signed_tx.hex),
        }
    }

    pub fn get_new_address(&self, label: String) -> Result<Address, Box<dyn std::error::Error>> {
        match self
            .bitcoind_client
            .get_new_address(Some(String::as_str(&label.clone())), None)
        {
            Ok(addr) => Ok(addr),
            Err(e) => Err("could not create new address".into()),
        }
    }

    pub fn create_wallet(&self, label: String) -> Result<(), Box<dyn std::error::Error>> {
        match self.bitcoind_client.create_wallet(
            String::as_str(&label.clone()),
            None,
            None,
            None,
            None,
        ) {
            Ok(res) => {
                if let Some(warning) = res.warning {
                    if warning != "" {
                        return Err(warning.into());
                    } else {
                        Ok(())
                    }
                } else {
                    Ok(())
                }
            }
            Err(e) => Err(e.into()),
        }
    }
}

impl BlockSource for &LdkBitcoindClient {
    fn get_header<'a>(
        &'a self,
        header_hash: &'a BlockHash,
        _height_hint: Option<u32>,
    ) -> AsyncBlockSourceResult<'a, BlockHeaderData> {
        Box::pin(async move {
            let res = self.bitcoind_client.get_block_header_info(header_hash);
            match res {
                Ok(res) => {
                    let converted_res = BlockHeaderData {
                        header: bitcoin::BlockHeader {
                            version: res.version,
                            prev_blockhash: res.previous_block_hash.unwrap(),
                            merkle_root: res.merkle_root,
                            time: res.time as u32,
                            bits: u32::from_str_radix(&res.bits, 16).unwrap(),
                            nonce: res.nonce,
                        },
                        height: res.height as u32,
                        chainwork: Uint256::from_be_bytes(res.chainwork.try_into().unwrap()),
                    };
                    Ok(converted_res)
                }
                // TODO verify error type
                Err(e) => Err(BlockSourceError::transient(e)),
            }
        })
    }

    fn get_block<'a>(&'a self, header_hash: &'a BlockHash) -> AsyncBlockSourceResult<'a, Block> {
        Box::pin(async move {
            let res = self.bitcoind_client.get_block(header_hash);
            match res {
                Ok(res) => Ok(res),
                // TODO verify error type
                Err(e) => Err(BlockSourceError::transient(e)),
            }
        })
    }

    fn get_best_block<'a>(&'a self) -> AsyncBlockSourceResult<(BlockHash, Option<u32>)> {
        Box::pin(async move {
            let res = self.bitcoind_client.get_blockchain_info();
            match res {
                Ok(res) => Ok((res.best_block_hash, Some(res.blocks as u32))),
                // TODO verify error type
                Err(e) => Err(BlockSourceError::transient(e)),
            }
        })
    }
}

const MIN_FEERATE: u32 = 253;

impl FeeEstimator for LdkBitcoindClient {
    fn get_est_sat_per_1000_weight(&self, confirmation_target: ConfirmationTarget) -> u32 {
        match confirmation_target {
            ConfirmationTarget::Background => {
                let res = self
                    .bitcoind_client
                    .estimate_smart_fee(144, Some(EstimateMode::Economical));
                match res {
                    Ok(res) => {
                        if let Some(fee_rate) = res.fee_rate {
                            std::cmp::max(MIN_FEERATE, (fee_rate.to_sat() / 4) as u32)
                        } else {
                            MIN_FEERATE
                        }
                    }
                    Err(_) => MIN_FEERATE,
                }
            }
            ConfirmationTarget::Normal => {
                let res = self
                    .bitcoind_client
                    .estimate_smart_fee(18, Some(EstimateMode::Conservative));
                match res {
                    Ok(res) => {
                        if let Some(fee_rate) = res.fee_rate {
                            std::cmp::max(MIN_FEERATE, (fee_rate.to_sat() / 4) as u32)
                        } else {
                            // TODO probably not min for normal
                            MIN_FEERATE
                        }
                    }
                    // TODO probably not min for normal
                    Err(_) => MIN_FEERATE,
                }
            }
            ConfirmationTarget::HighPriority => {
                let res = self
                    .bitcoind_client
                    .estimate_smart_fee(6, Some(EstimateMode::Conservative));
                match res {
                    Ok(res) => {
                        if let Some(fee_rate) = res.fee_rate {
                            std::cmp::max(MIN_FEERATE, (fee_rate.to_sat() / 4) as u32)
                        } else {
                            // TODO probably not min for high
                            MIN_FEERATE
                        }
                    }
                    // TODO probably not min for high
                    Err(_) => MIN_FEERATE,
                }
            }
        }
    }
}

impl BroadcasterInterface for LdkBitcoindClient {
    fn broadcast_transaction(&self, tx: &Transaction) {
        let res = self.bitcoind_client.send_raw_transaction(tx);
        // This may error due to RL calling `broadcast_transaction` with the same transaction
        // multiple times, but the error is safe to ignore.
        match res {
            Ok(_) => {}
            Err(e) => {
                let err_str = e.to_string();
                if !err_str.contains("Transaction already in block chain")
                    && !err_str.contains("Inputs missing or spent")
                    && !err_str.contains("bad-txns-inputs-missingorspent")
                    && !err_str.contains("txn-mempool-conflict")
                    && !err_str.contains("non-BIP68-final")
                    && !err_str.contains("insufficient fee, rejecting replacement ")
                {
                    panic!("{}", e);
                }
            }
        }
    }
}
