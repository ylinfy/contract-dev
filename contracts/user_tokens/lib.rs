#![cfg_attr(not(feature = "std"), no_std)]

pub use self::user_tokens::UserTokens;

#[brush::contract]
mod user_tokens {
    use brush::modifiers;
    use ownable::traits::*;

    use ink_storage::{
        collections::HashMap as StorageHashMap,
        collections::hashmap::Entry,
    };
    use ink_prelude::collections::{
        BTreeMap, 
        btree_map::Entry as BEntry
    };

    pub type Id = [u8; 32];

    #[ink(storage)]
    #[derive(Default, OwnableStorage)]
    pub struct UserTokens {
        #[OwnableStorageField]
        ownable: OwnableData,
        /// Mapping from AccountId to Map(Mapping token id to index);
        user_token_ids: StorageHashMap<AccountId, BTreeMap<Id, bool>>,
        internal_callers: StorageHashMap<AccountId, bool>
    }

    impl Ownable for UserTokens {}

    impl UserTokens {
        #[ink(constructor)]
        pub fn new() -> Self {
            let mut instance = Self::default();
            let caller = Self::env().caller();
            instance._init_with_owner(caller);
            instance
        }

        #[ink(message)]
        #[modifiers(only_owner)]
        pub fn set_internal_caller(&mut self, user: AccountId, is: bool) -> Result<(), OwnableError> {
            self.internal_callers.insert(user, is);
            Ok(())
        }

        #[ink(message)]
        pub fn is_internal_caller(&self) -> bool {
            let caller = self.env().caller();
            self.internal_callers.get(&caller).copied().unwrap_or(false)
        }

        #[ink(message)]
        pub fn add_user_tokens(&mut self, user: AccountId, token_id: Id) {
            assert!(self.is_internal_caller(), "not internal caller");
            match self.user_token_ids.entry(user) {
                Entry::Vacant(vacant) => {
                    let mut _bmap = BTreeMap::new();
                    _bmap.insert(token_id, true);
                    vacant.insert(_bmap);
                },
                Entry::Occupied(mut occupied) => {
                    let mut _bmap = occupied.get_mut();
                    match _bmap.entry(token_id) {
                        BEntry::Vacant(vacant) => { vacant.insert(true); },
                        _ => (), // TODO, already exist, do nothing.
                    }
                },
            }
        }

        #[ink(message)]
        pub fn delete_user_token(&mut self, user: AccountId, token_id: Id) {
            assert!(self.is_internal_caller(), "not internal caller");
            match self.user_token_ids.entry(user) {
                Entry::Vacant(_) => (),  // TODO: if it is empty, do nothing.
                Entry::Occupied(mut occupied) => {
                    let mut _bmap = occupied.get_mut();
                    match _bmap.entry(token_id) {
                        BEntry::Vacant(_) => (),  // TODO: if not has this token, do nothing.
                        BEntry::Occupied(boccupied) => { boccupied.remove_entry();},
                    }
                },
            }
        }
    }
}
