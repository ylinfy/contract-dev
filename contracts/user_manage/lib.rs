#![cfg_attr(not(feature = "std"), no_std)]

pub use self::user_manage::UserManage;

#[brush::contract]
mod user_manage {
    use nft_factory::NftFactory;
    use brush::modifiers;
    use ownable::traits::*;
    use psp1155::traits::PSP1155AsDependency;

    use ink_storage::{
        lazy::Lazy,
        collections::hashmap::Entry,
        collections::HashMap as StorageHashMap,
        traits::{SpreadLayout, PackedLayout},
    };
    use scale::{Encode, Decode};
    use ink_prelude::string::String;
    use ink_env::call::FromAccountId;

    pub type Id = [u8; 32];

    #[derive(Default, Debug, PartialEq, Eq, Encode, Decode, SpreadLayout, PackedLayout)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct User {
        name: String,
        desc: String,  // description of user
        avatar_nft_id: Id,
        status: u8,
        is_registered: bool, 
    }


    #[ink(storage)]
    #[derive(Default, OwnableStorage)]
    pub struct UserManage {
        #[OwnableStorageField]
        ownable: OwnableData,
        /// nft factory contract
        nft: Lazy<NftFactory>,
        /// total users.
        total_users: u128,
        /// Mapping from `AccountId` to User Info.
        users: StorageHashMap<AccountId, User>,
        /// Mapping from `AccountId` to a bool value.
        managers: StorageHashMap<AccountId, bool>,
    }

    impl Ownable for UserManage {}

    impl UserManage {
        #[ink(constructor)]
        pub fn new() -> Self {
            let mut instance = Self::default();
            let caller = Self::env().caller();
            instance._init_with_owner(caller);
            instance
        }

        #[ink(message)]
        #[modifiers(only_owner)]
        pub fn init_nft_factory(&mut self, token1155: AccountId) -> Result<(), OwnableError> {
            let _nft: NftFactory = FromAccountId::from_account_id(token1155);
            self.nft = Lazy::new(_nft); 
            Ok(())
        }

        #[ink(message)]
        pub fn update_user_info(
            &mut self, 
            _name: String, 
            _desc: String, 
            _avatar_nft_id: Id,
        ) -> bool {
            let caller = self.env().caller();
            let _user = User {
                name: _name.clone(),
                desc: _desc.clone(),
                avatar_nft_id: _avatar_nft_id,
                status: 0,
                is_registered: true,
            };

            match self.users.entry(caller) {
                Entry::Vacant(vacant) => {
                    self.total_users += 1;
                    vacant.insert(_user);
                },
                Entry::Occupied(mut occupied) => {
                    let occ_user = occupied.get_mut();
                    occ_user.name = _name;
                    occ_user.desc = _desc;
                    if _avatar_nft_id != [0; 32] {
                        assert!(self.nft.balance_of(caller, _avatar_nft_id) > 0, "caller has not this NFT");
                        occ_user.avatar_nft_id = _avatar_nft_id;
                    }
                },
            }
            true
        }

        #[ink(message)]
        pub fn modify_user_status(&mut self, user: AccountId, status: u8) -> bool {
            let caller = self.env().caller();
            assert!(self.is_manager(caller), "not manager");
            match self.users.entry(user) {
                Entry::Vacant(_) => return false,
                Entry::Occupied(mut occupied) => {
                    let occ_user = occupied.get_mut();
                    occ_user.status = status;
                },
            }
            true
        }

        #[ink(message)]
        #[modifiers(only_owner)]
        pub fn set_manager(&mut self, _user: AccountId, _is_manager: bool) -> Result<(), OwnableError> {
            self.managers.insert(_user, _is_manager);
            Ok(())
        }

        #[ink(message)]
        pub fn is_manager(&self, _user: AccountId) -> bool {
            // default: false
            self.managers.get(&_user).copied().unwrap_or(false)
        }
    }
}
