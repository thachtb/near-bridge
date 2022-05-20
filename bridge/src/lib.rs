/*!
Near - Incognito bridge implementation with JSON serialization.
NOTES:
  - Shield / Unshield features: move tokens forth and back between Near and Incognito
  - Swap beacon
*/

mod token_receiver;
mod errors;
mod utils;

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{
    env, near_bindgen, BorshStorageKey, PanicOnDefault, ext_contract
};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::collections::{LookupMap, TreeMap};
use crate::errors::*;
use crate::utils::{NEAR_ADDRESS, LEN};
use crate::utils::{verify_inst};
use arrayref::{array_refs, array_ref};
use near_contract_standards::fungible_token::metadata::FungibleTokenMetadata;

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct InteractRequest {
    // instruction in bytes
    pub inst: String,
    // beacon height
    pub height: u128,
    // inst paths to build merkle tree
    pub inst_paths: Vec<[u8; 32]>,
    // inst path indicator
    pub inst_path_is_lefts: Vec<bool>,
    // instruction root
    pub inst_root: [u8; 32],
    // blkData
    pub blk_data: [u8; 32],
    // signature index
    pub indexes: Vec<u8>,
    // signatures
    pub signatures: Vec<String>,
    // v value
    pub vs: Vec<u8>
}

#[derive(BorshStorageKey, BorshSerialize)]
pub(crate) enum StorageKey {
    Transaction,
    BeaconHeight,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Vault {
    // mark tx already burn
    pub tx_burn: LookupMap<[u8; 32], bool>,
    // beacon committees
    pub beacons: TreeMap<u128, Vec<String>>,
}

// define the methods we'll use on ContractB
#[ext_contract(ext_ft)]
pub trait FtContract {
    fn ft_metadata(&self) -> FungibleTokenMetadata;
    fn ft_balance_of(&self) -> String;
}

// define methods we'll use as callbacks on ContractA
#[ext_contract(ext_self)]
pub trait VaultContract {
    fn deposit_ft_callback(&self) -> String;
}

#[near_bindgen]
impl Vault {
    /// Initializes the beacon list
    #[init]
    pub fn new(
        beacons: Vec<String>,
        height: u128,
    ) -> Self {
        assert!(!env::state_exists(), "Already initialized");
        assert!(beacons.len().eq(&0), "Invalid beacon list");
        let mut this = Self {
            tx_burn: LookupMap::new(StorageKey::Transaction), 
            beacons: TreeMap::new(StorageKey::BeaconHeight)
        };
        // insert beacon height and list in tree
        this.beacons.insert(&height, &beacons);

        this
    }

    /// shield native token
    ///
    /// receive token from users and generate proof
    /// validate proof on Incognito side and mint corresponding token
    #[payable]
    pub fn deposit(
        &mut self,
        incognito_address: String,
    ) {
        // extract near amount from deposit transaction
        let amount = env::attached_deposit().checked_div(1e15 as u128).unwrap_or(0);
        env::log_str(format!(
            "{} {} {}",
            incognito_address, NEAR_ADDRESS.to_string(), amount
        ).as_str());
    }

    /// withdraw tokens
    ///
    /// submit burn proof to receive token
    pub fn withdraw(
        &mut self,
        unshield_info: InteractRequest
    ) -> bool {
        let beacons = self.get_beacons(unshield_info.height);

        // verify instruction
        verify_inst(&unshield_info, beacons);

        // parse instruction
        let inst = hex::decode(unshield_info.inst).unwrap_or_default();
        let inst_ = array_ref![inst, 0, LEN];
        #[allow(clippy::ptr_offset_with_cast)]
        let (meta_type, shard_id, _, token, _, receiver_key, _, unshield_amount, tx_id) =
            array_refs![inst_, 1, 1, 12, 20, 12, 20, 24, 8, 32];
        let meta_type = u8::from_le_bytes(*meta_type);
        let shard_id = u8::from_le_bytes(*shard_id);
        let mut unshield_amount = u128::from(u64::from_be_bytes(*unshield_amount));

        // validate metatype and key provided
        if (meta_type != 157 && meta_type != 158) || shard_id != 1 {
            panic!("{}", INVALID_METADATA);
        }

        // check tx burn used
        if self.tx_burn.get(&tx_id).unwrap_or_default() {
            panic!("{}", INVALID_TX_BURN);
        }
        self.tx_burn.insert(&tx_id, &true);


        // todo: transfer token to users.

        true
    }

    pub fn swap_beacon_committee(
        &mut self,
        swap_info: InteractRequest
    ) {
        let beacons = self.get_beacons(swap_info.height);

        // verify instruction
        verify_inst(&swap_info, beacons);
        
        // todo: parse instruction
    }


    /// getters

    /// get beacon list by height
    pub fn get_beacons(&self, height: u128) -> Vec<String> {
        let get_height_key = self.beacons.lower(&height).unwrap();
        self.beacons.get(&get_height_key).unwrap()
    }

    /// check tx burn used
    pub fn get_tx_burn_used(self, tx_id: &[u8; 32]) -> bool {
        self.tx_burn.get(tx_id).unwrap_or_default()
    }
}
