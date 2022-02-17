#![cfg_attr(not(feature = "std"), no_std)]
#![feature(min_specialization)]

#[brush::contract]
mod id_provider {
    use brush::{
        contracts::{ownable::*, psp1155::*},
        modifiers,
    };
    use ink_storage::{
        lazy::Lazy,
        collections::HashMap as StorageHashMap,
        traits::{SpreadLayout, PackedLayout},
    };
    use scale::{Encode, Decode};

    #[cfg(feature = "std")]
    use ink_storage::traits::StorageLayout;

    #[derive(Default, Clone, Debug, PartialEq, Eq, Encode, Decode, SpreadLayout, PackedLayout)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
    pub struct NFT {
        token_address: AccountId,
        token_id: Id,
        amount: u128,
        owner: AccountId,
        nft_type: u32,
        is_in_collection: bool,
    }

    #[ink(storage)]
    #[derive(Default, OwnableStorage, PSP1155Storage)]
    pub struct IdProvider {
        #[OwnableStorageField]
        ownable: OwnableData,
        #[PSP1155StorageField]
        psp1155: PSP1155Data,
        nft_id: u128,
        all_nfts: StorageHashMap<u128, NFT>,
        nfts_of_owner: StorageHashMap<AccountId, Vec<NFT>>,
        nft_id_map: StorageHashMap<(AccountId, Id, AccountId), u128>,
        internal_caller: StorageHashMap<AccountId, bool>,
    }

    impl Counter {
        pub fn _current(&self) -> u128 {
            self.id
        }

        pub fn _increase(&mut self) -> u128 {
            self.id += 1;
            self.id
        }
    }

    impl Ownable for IdProvider {}

    impl IdProvider {
        #[ink(constructor)]
        pub fn new() -> Self {
            let mut instance = Self::default();
            let caller = instance.env().caller();
            instance._init_with_owner(caller);
            instance
        }

        #[ink(message)]
        #[modifiers(only_owner)]
        pub fn set_internal_caller(
            &mut self, 
            user: AccountId, 
            value: bool
        ) -> Result<(), OwnableError> {
            self.internal_caller.insert(user, value);
            Ok(())
        }

        // all id stats from 1. 
        // Optimize the two fns to change its beginning to zero if necessary.
        #[ink(message)]
        pub fn new_work_id(&mut self) -> u128 {
            self.only_internal_caller();
            self.work_id._increase()
        }

        #[ink(message)]
        pub fn new_collection_id(&mut self) -> u128 {
            self.only_internal_caller();
            let new_id = self.collection_id._increase();

            // TODO: for test (replace the choice with base and virtual base contract address)
            // let caller = self.env().caller();
            // match caller {
            //     base_contract if new_id % 2 == 0 => new_id + 1,
            //     base_contract => new_id,
            //     _ if new_id % 2 == 0 => new_id,
            //     _ => new_id + 1,
            // }
            let choice = 1;
            match choice {
                1 if new_id % 2 == 0 => self.collection_id._increase(),
                1 => new_id,
                _ if new_id % 2 == 0 => new_id,
                _ => self.collection_id._increase(),
            }
        }

        #[ink(message)]
        pub fn work_id(&self) -> u128 {
            self.work_id._current()
        }

        #[ink(message)]
        pub fn collection_id(&self) -> u128 {
            self.collection_id._current()
        }

        fn only_internal_caller(&self) {
            let caller = self.env().caller();
            assert!(self.internal_caller.get(&caller).unwrap());
        }
    }
}
