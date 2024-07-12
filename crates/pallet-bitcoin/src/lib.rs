//! # Bitcoin Pallet
//!
//! This pallet is designed to be minimalist, containing only one storage item for maintaining the state
//! of the UTXO (Unspent Transaction Output) set by processing the inputs and outputs of each Bitcoin
//! transaction wrapped in [`Call::transact`]. There is no verification logic within the
//! pallet, all validation work should be performed outside the runtime. This approach simplifies
//! off-runtime execution, allowing for easier syncing performance optimization.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;

use bitcoin::consensus::{Decodable, Encodable};
use bitcoin::{OutPoint, Transaction as BitcoinTransaction};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::dispatch::DispatchResult;
use frame_support::weights::Weight;
use scale_info::TypeInfo;
use sp_core::H256;
use sp_std::prelude::*;
use sp_std::vec::Vec;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

const MAX_SCRIPT_SIZE: usize = 10_000;

/// Transaction output index.
pub type Vout = u32;

/// Wrapper type for Bitcoin txid in runtime as `bitcoin::Txid` does not implement codec.
#[derive(Clone, TypeInfo, Encode, Decode, MaxEncodedLen)]
pub struct Txid(H256);

impl Txid {
    fn from_bitcoin_txid(txid: bitcoin::Txid) -> Self {
        let mut d = Vec::new();
        txid.consensus_encode(&mut d)
            .expect("txid must be encoded correctly; qed");

        let d: [u8; 32] = d
            .try_into()
            .expect("Bitcoin txid is sha256 hash which must fit into [u8; 32]; qed");

        Self(H256::from(d))
    }

    pub fn into_bitcoin_txid(self) -> bitcoin::Txid {
        bitcoin::consensus::Decodable::consensus_decode(&mut self.encode().as_slice())
            .expect("txid must be encoded correctly; qed")
    }
}

impl core::fmt::Debug for Txid {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for byte in self.0.as_bytes().iter().rev() {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

/// Unspent transaction output.
#[derive(Debug, TypeInfo, Encode, Decode)]
pub struct Coin {
    /// Whether the coin is from a coinbase transaction.
    pub is_coinbase: bool,
    /// Transfer value in satoshis.
    pub amount: u64,
    /// Spending condition of the output.
    pub script_pubkey: Vec<u8>,
}

impl MaxEncodedLen for Coin {
    fn max_encoded_len() -> usize {
        bool::max_encoded_len() + u64::max_encoded_len() + MAX_SCRIPT_SIZE
    }
}

#[derive(Debug, TypeInfo, Encode, Decode, MaxEncodedLen)]
struct OutPointInner {
    txid: Txid,
    vout: Vout,
}

impl From<OutPoint> for OutPointInner {
    fn from(out_point: OutPoint) -> Self {
        Self {
            txid: Txid::from_bitcoin_txid(out_point.txid),
            vout: out_point.vout,
        }
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type WeightInfo: frame_system::WeightInfo;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::call(weight(<T as Config>::WeightInfo))]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::zero())]
        pub fn transact(origin: OriginFor<T>, btc_tx: Vec<u8>) -> DispatchResult {
            ensure_none(origin)?;

            let bitcoin_transaction = Self::decode_transaction(btc_tx);
            Self::process_bitcoin_transaction(bitcoin_transaction);

            Ok(())
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T> {
        pub genesis_tx: Vec<u8>,
        pub _config: core::marker::PhantomData<T>,
    }

    // Custom Default impl to make `test_genesis_config_builds()` in runtime happy.
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            // `genesis_tx` is generated by
            // `subcoin_primitives::bitcoin_genesis_tx(bitcoin::Network::Bitcoin)`.
            let genesis_tx = [
                1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255, 77, 4, 255, 255, 0, 29, 1, 4, 69,
                84, 104, 101, 32, 84, 105, 109, 101, 115, 32, 48, 51, 47, 74, 97, 110, 47, 50, 48,
                48, 57, 32, 67, 104, 97, 110, 99, 101, 108, 108, 111, 114, 32, 111, 110, 32, 98,
                114, 105, 110, 107, 32, 111, 102, 32, 115, 101, 99, 111, 110, 100, 32, 98, 97, 105,
                108, 111, 117, 116, 32, 102, 111, 114, 32, 98, 97, 110, 107, 115, 255, 255, 255,
                255, 1, 0, 242, 5, 42, 1, 0, 0, 0, 67, 65, 4, 103, 138, 253, 176, 254, 85, 72, 39,
                25, 103, 241, 166, 113, 48, 183, 16, 92, 214, 168, 40, 224, 57, 9, 166, 121, 98,
                224, 234, 31, 97, 222, 182, 73, 246, 188, 63, 76, 239, 56, 196, 243, 85, 4, 229,
                30, 193, 18, 222, 92, 56, 77, 247, 186, 11, 141, 87, 138, 76, 112, 43, 107, 241,
                29, 95, 172, 0, 0, 0, 0,
            ];

            Self {
                genesis_tx: genesis_tx.to_vec(),
                _config: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            let genesis_tx = Pallet::<T>::decode_transaction(self.genesis_tx.clone());

            let txid = Txid::from_bitcoin_txid(genesis_tx.compute_txid());

            genesis_tx
                .output
                .iter()
                .enumerate()
                .for_each(|(index, txout)| {
                    let coin = Coin {
                        is_coinbase: true,
                        amount: txout.value.to_sat(),
                        script_pubkey: txout.script_pubkey.clone().into_bytes(),
                    };
                    Coins::<T>::insert(txid.clone(), index as u32, coin);
                });
        }
    }

    #[pallet::event]
    pub enum Event<T: Config> {}

    /// UTXO set.
    ///
    /// (Txid, Vout, Coin)
    #[pallet::storage]
    pub type Coins<T> = StorageDoubleMap<_, Identity, Txid, Identity, Vout, Coin, OptionQuery>;
}

/// Returns the storage key for the referenced output.
pub fn coin_storage_key<T: Config>(bitcoin_txid: bitcoin::Txid, index: Vout) -> Vec<u8> {
    use frame_support::storage::generator::StorageDoubleMap;

    let txid = Txid::from_bitcoin_txid(bitcoin_txid);
    Coins::<T>::storage_double_map_final_key(txid, index)
}

pub fn coin_storage_prefix<T: Config>() -> [u8; 32] {
    use frame_support::StoragePrefixedMap;

    Coins::<T>::final_prefix()
}

impl<T: Config> Pallet<T> {
    fn decode_transaction(btc_tx: Vec<u8>) -> BitcoinTransaction {
        BitcoinTransaction::consensus_decode(&mut btc_tx.as_slice()).unwrap_or_else(|_| {
            panic!("Transaction constructed internally must be decoded successfully; qed")
        })
    }

    fn process_bitcoin_transaction(tx: BitcoinTransaction) {
        let txid = tx.compute_txid();
        let is_coinbase = tx.is_coinbase();

        let new_coins = tx
            .output
            .into_iter()
            .enumerate()
            .map(|(index, txout)| {
                let out_point = OutPoint {
                    txid,
                    vout: index as u32,
                };
                let coin = Coin {
                    is_coinbase,
                    amount: txout.value.to_sat(),
                    script_pubkey: txout.script_pubkey.into_bytes(),
                };

                (out_point, coin)
            })
            .collect::<Vec<_>>();

        if is_coinbase {
            for (out_point, coin) in new_coins {
                let OutPointInner { txid, vout } = OutPointInner::from(out_point);
                Coins::<T>::insert(txid, vout, coin);
            }
            return;
        }

        // Process inputs.
        for input in tx.input {
            let previous_output = input.previous_output;
            let OutPointInner { txid, vout } = OutPointInner::from(previous_output);
            if let Some(_spent) = Coins::<T>::take(txid, vout) {
            } else {
                panic!("Corruputed state, UTXO {previous_output:?} not found");
            }
        }

        // Process outputs.
        for (out_point, coin) in new_coins {
            let OutPointInner { txid, vout } = OutPointInner::from(out_point);
            Coins::<T>::insert(txid, vout, coin);
        }
    }
}
