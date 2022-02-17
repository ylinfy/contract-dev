#![cfg_attr(not(feature = "std"), no_std)]

pub use self::nft_factory::NftFactory;

#[brush::contract]
mod nft_factory {
    // TODO: use user_tokens::UserTokens; and add user's tokens' changing corresponding codes.
    // TODO: override transfer because transfer will take change to UserTokens.
    use psp1155::traits::*;

    #[cfg(not(feature = "ink-as-dependency"))]
    use ink_storage::collections::HashMap as StorageHashMap;
    #[cfg(not(feature = "ink-as-dependency"))]
    use ink_prelude::vec;

    use ink_storage::traits::{SpreadLayout, PackedLayout};
    use ink_prelude::{vec::Vec, string::String};
    use scale::{Encode, Decode};

    #[ink(event)]
    pub struct NFTMinted {
        #[ink(topic)]
        origin_id: Id,
        #[ink(topic)]
        fragment_ids_amounts: Vec<(Id, Balance)>,
        #[ink(topic)]
        num_fragments: u128,
        num_whole_copies: u128,
    }

    #[ink(event)]
    pub struct NFTMerged {
        #[ink(topic)]
        origin_id: Id,
        #[ink(topic)]
        quantity: u128,
    }

    #[derive(Debug, PartialEq, Eq, Encode, Decode, SpreadLayout, PackedLayout)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct Work {
        uri: String,
        num_fragments: u128,
    }

    #[derive(Default, PSP1155Storage)]
    #[ink(storage)]
    pub struct NftFactory {
        #[PSP1155StorageField]
        psp1155: PSP1155Data,
        id: Id,
        uri: String,
        mystery_box_government: AccountId,
        origin_id_to_work: StorageHashMap<Id, Work>,
        is_fragments: StorageHashMap<Id, bool>,
    }

    impl PSP1155 for NftFactory {}

    impl NftFactory {
        #[ink(constructor)]
        pub fn new() -> Self {
            // TODO: need pass the `mysteryBox` contract address argment to replace the `mb_addr`.
            let mut instance = Self::default();
            let mb_addr: AccountId = [1; 32].into();
            instance.mystery_box_government = mb_addr;
            instance.id = [0; 32];
            instance
        }
        
        #[ink(message)]
        pub fn mint(
            &mut self, 
            _mb_market_addr: AccountId,
            _uri: String,
            _num_full_copies: u128,
            _num_split_full_copies: u128,
            _num_fragments: u128,
        ) -> Result<(Id, Vec<(Id, Balance)>), PSP1155Error> {
            assert!(
                self.mystery_box_government() == self.env().caller(), 
                "only mystery box government contract authorized",
            );
            // get origin id.
            let mut _origin_id = self.current_id();
            self.id = self.increase_id(_origin_id, 16); // first half [u8; 16] use for OriginId
            _origin_id = self.current_id();

            // get fragment_ids_amounts
            let mut _fragment_ids_amounts = Vec::new();
            let mut _fragment_id = _origin_id;  // FragmentId consists of OriginId + Index
            for _ in 0.._num_fragments {
                _fragment_id = self.increase_id(_fragment_id, 32); // second half [u8; 16] use for FragmentId Index 
                _fragment_ids_amounts.push((_fragment_id, _num_split_full_copies));
                self.is_fragments.insert(_fragment_id, true);
            }

            // mint fragments
            self._mint_to(_mb_market_addr, _fragment_ids_amounts.clone())?;

            // mint origin
            if _num_full_copies > 0 {
                self._mint_to(_mb_market_addr, vec![(_origin_id, _num_full_copies)])?;
                self.set_uri(_uri.clone());
            }

            // update state variable
            let _work = Work { uri: _uri, num_fragments: _num_fragments };
            self.origin_id_to_work.insert(_origin_id, _work);

            self.env().emit_event( NFTMinted {
                origin_id: _origin_id,
                fragment_ids_amounts: _fragment_ids_amounts.clone(),
                num_fragments: _num_fragments,
                num_whole_copies: _num_full_copies
            });

            Ok((_origin_id, _fragment_ids_amounts))
        }

        #[ink(message)]
        pub fn merge(&mut self, _origin_id: Id, _quantity: u128) -> Result<(), PSP1155Error> {
            assert!(_quantity > 0, "quantity is zero");
            let caller = self.env().caller();
            let _works = self.origin_id_to_work.get(&_origin_id)
                .expect("the origin id doesn't exist");
            let _num_fragments = _works.num_fragments;
            let _uri = _works.uri.clone();

            // get fragments ids_quantities
            let mut _fragment_ids_quantities = Vec::new();
            let mut _fragment_id = _origin_id;  // FragmentId consists of OriginId + Index
            for _ in 0.._num_fragments {
                _fragment_id = self.increase_id(_fragment_id, 32);
                if self.balance_of(caller, _fragment_id) < _quantity {
                    return Err(PSP1155Error::InsufficientBalance);
                }
                _fragment_ids_quantities.push((_fragment_id, _quantity));
            }
            // burn fragments and mint origin
            self._burn_from(caller, _fragment_ids_quantities)?;
            self._mint_to(caller, vec![(_origin_id, _quantity)])?;

            // TODO: why does it need to set uri, what does the state variable `uri` use for?
            self.set_uri(_uri);

            self.env().emit_event( NFTMerged {
                origin_id: _origin_id,
                quantity: _quantity,
            });
            Ok(())
        }

        #[ink(message)]
        pub fn get_token_info(&self, _token_id: u128) {
            // TODO: complete this function according to its caller.
        }

        #[ink(message)]
        pub fn is_fragment(&self, _token_id: Id) -> bool {
            *self.is_fragments.get(&_token_id).unwrap_or(&false)
        }

        #[ink(message)]
        pub fn mystery_box_government(&self) -> AccountId {
            self.mystery_box_government
        }
    }
    
    // private functions
    impl NftFactory {
        // the first 16 u8 elements, use to present complete NFT.
        // the second 16 u8 elements, use to present fragment NFT's index.
        // 00000000000000010000000000000000: originId (with 3 fragments)
        // 00000000000000010000000000000001: fragmentId1
        // 00000000000000010000000000000002: fragmentId2
        // 00000000000000010000000000000003: fragmentId3
        fn increase_id(&mut self, _id: Id, _i: u8) -> Id {
            let (mut _id, mut _i) = (_id, _i);
            for _ in 0..15 {
                if _id[_i as usize -1] != u8::MAX {
                    _id[_i as usize -1] += 1;
                    break;
                }
                _i -= 1;
            }
            _id
        }

        fn current_id(&self) -> Id {
            self.id
        }

        fn set_uri(&mut self, _uri: String) {
            self.uri = _uri;
        }
    }
}
