#![cfg_attr(not(feature = "std"), no_std)]


#[brush::contract]
mod lucky_lottery {
    use draw_lots::DrawLots;
    use ink_env::call::FromAccountId;
    use brush::modifiers;
    use ownable::traits::*;

    use ink_storage::{
        lazy::Lazy,
        collections::HashMap as StorageHashMap,
        collections::hashmap::Entry,
        traits::{SpreadLayout, PackedLayout},
    };
    use ink_prelude::{
        vec::Vec,
        string::String,
        collections::BTreeMap,
        collections::btree_map::Entry as BEntry,
    };
    use scale::{Encode, Decode};

    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        Custom(String),
        NotFound,
    }

    impl From<OwnableError> for Error {
        fn from(err: OwnableError) -> Self {
            match err {
                OwnableError::CallerIsNotOwner => Error::Custom(String::from("O::CallerIsNotOwner")),
                OwnableError::NewOwnerIsZero => Error::Custom(String::from("O::NewOwnerIsZero")),
            }
        }
    }

    #[derive(Debug, PartialEq, Eq, Encode, Decode, SpreadLayout, PackedLayout)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct UserInfo {
        buy_quantity: u128,
        // (start index, end index) to is received reward.
        sections: BTreeMap<(u128, u128), bool>,
        reward_amount: BTreeMap<AccountId, u128>,
    }

    #[derive(Debug, PartialEq, Eq, Encode, Decode, SpreadLayout, PackedLayout)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct Lottery {
        winning_quantity: u128, // number of winners.
        total_quantity: u128, // all lots
        reward_ratio: u128,  // suggest 10000
        winning_tails: BTreeMap<u128, u8>,  // all winning tails.
        reward_amount: BTreeMap<AccountId, u128>,  // token addr to amount
        user_infos: BTreeMap<AccountId, UserInfo>, // user addr to user's info
    }

    #[ink(storage)]
    #[derive(Default, OwnableStorage)]
    pub struct LuckyLottery {
        #[OwnableStorageField]
        ownable: OwnableData,
        min_lottery_addr_quantities: StorageHashMap<u128, u128>,
        /// Mapping from (pool_id, token addr) to amount
        remain_amounts: StorageHashMap<u128, BTreeMap<AccountId, u128>>,
        // Mapping from pool_id to Map(lottery times to Lottery)
        lotteries: StorageHashMap<u128, BTreeMap<u128, Lottery>>,
        mystery_box_market: AccountId,
        token20s: StorageHashMap<AccountId, bool>,
        draw_lots: Lazy<DrawLots>,
    }

    impl Ownable for LuckyLottery {}

    impl LuckyLottery {
        #[ink(constructor)]
        pub fn new(draw_lots: AccountId) -> Self {
            // TODO: don't forget to add the contract or account address.
            let mut instance = Self::default();
            let caller = Self::env().caller();
            instance._init_with_owner(caller);
            instance.mystery_box_market = caller;
            let mut min_requires = StorageHashMap::new();
            min_requires.insert(0, 500000);
            instance.min_lottery_addr_quantities = min_requires;

            let draw_lots = FromAccountId::from_account_id(draw_lots);
            instance.draw_lots = Lazy::new(draw_lots);
            instance
        }

        #[ink(message)]
        #[modifiers(only_owner)]
        pub fn adjust_min_lottery_addr_quantity_of_pool(
            &mut self, 
            pool_id: u128, 
            quantity: u128
        ) -> Result<(), OwnableError> {
            self.min_lottery_addr_quantities.insert(pool_id, quantity);
            Ok(())
        }

        #[ink(message)]
        pub fn add_reward_token(&mut self, token20: AccountId) {
            self.only_mb_market();
            match self.token20s.entry(token20) {
                Entry::Vacant(vacant) => { vacant.insert(true); },
                Entry::Occupied(_) => (),
            }
        }

        #[ink(message)]
        pub fn add_lottery_data(
            &mut self,
            pool_id: u128,
            user: AccountId,
            quantity: u128,
            token20: AccountId,
            amount: u128,
        ) -> Result<(), Error> {
            self.only_mb_market();
            match self.lotteries.entry(pool_id) {
                Entry::Vacant(vacant) => {
                    let mut lot_pool: BTreeMap<u128, Lottery> = BTreeMap::new();
                    let mut lot = Lottery::new();
                    // init lot
                    lot.total_quantity = quantity;
                    let mut user_info = UserInfo::new();
                    modify_user_info(&mut user_info, lot.total_quantity, quantity);
                    lot.user_infos.insert(user, user_info);
                    // init lot_pool, lot_times starts from 1
                    lot_pool.insert(1, lot);
                    vacant.insert(lot_pool);
                }, 
                Entry::Occupied(mut occupied) => {
                    let lot_pool = occupied.get_mut();
                    let lot_times = lot_pool.len() as u128;
                    match lot_pool.entry(lot_times) {
                        BEntry::Vacant(_) => return Err(Error::NotFound),
                        BEntry::Occupied(mut boccupied) => {
                            let lot = boccupied.get_mut();
                            lot.total_quantity += quantity;
                            match lot.user_infos.entry(user) {
                                BEntry::Vacant(vacant) => {
                                    let mut user_info = UserInfo::new();
                                    modify_user_info(&mut user_info, lot.total_quantity, quantity);
                                    vacant.insert(user_info);
                                },
                                BEntry::Occupied(mut uoccupied) => {
                                    let user_info = uoccupied.get_mut();
                                    modify_user_info(user_info, lot.total_quantity, quantity);
                                },
                            };
                        },
                    };
                },
            };
            self.remain_amounts.entry(pool_id).and_modify(|btmap| {
                btmap.entry(token20).and_modify(|v| *v += amount).or_insert(amount);
            }).or_insert(BTreeMap::from([(token20, amount)]));
            Ok(())
        }

        #[ink(message)]
        #[modifiers(only_owner)]
        pub fn draw_lottery(
            &mut self,
            pool_id: u128,
            reward_ratio: u128,
            salt: u32,
            win_quantity: u128,
        ) -> Result<(), Error> {
            let lot_times = self.lot_times(pool_id);
            let total_quantity = self.total_quantity(pool_id, lot_times);
            assert!(total_quantity >= self.min_lottery_addr_quantity(pool_id),
                "the amount of lottery addresses is too small");

            let (winning_tails, _) = self.draw_lots.draw_lots(salt, win_quantity, total_quantity);
            let reward_amount = match self.remain_amounts.entry(pool_id) {
                Entry::Vacant(_) => return Err(Error::NotFound),
                Entry::Occupied(mut occupied) => {
                    let mut reward_amount = BTreeMap::new();
                    let btmap = occupied.get_mut();
                    for (k, v) in btmap {
                        let reward = (*v).saturating_mul(reward_ratio).saturating_div(10000);
                        *v -= reward;  // TODO: if *v == 0, should it remove this (K, V)?
                        reward_amount.insert(*k, reward);
                    } reward_amount
                },
            };
            self.lotteries.entry(pool_id).and_modify(move |btmap| {
                btmap.entry(lot_times).and_modify(move |lot| {
                    lot.winning_quantity = win_quantity;
                    lot.reward_ratio = reward_ratio;
                    lot.winning_tails = winning_tails;
                    lot.reward_amount = reward_amount;
                });
                // lot_times++
                btmap.insert(lot_times + 1, Lottery::new());
            });

            Ok(())
        }

        #[ink(message)]
        pub fn receive_reward(&mut self, pool_id: u128, lot_times: u128, buy_times: u128) {
            let caller = self.env().caller();
            assert!(lot_times > 0 && lot_times < self.lot_times(pool_id), "invalid lottery times");
            assert!(buy_times > 0 && buy_times < self.buy_times(pool_id, lot_times, caller), "invalid buy times");
            let (start, end, is_received) = self.section_info(pool_id, lot_times, caller, buy_times);
            assert!(!is_received, "already received");  // is_received is false means available

            let (winning_tails, reward_amount, total_win_quantity) = self.lottery_info(pool_id, lot_times);

            // TODO: optimize this loop
            let mut winning_quantity = 0u128;
            for num in start..end + 1 {
                for (tail, pos) in &winning_tails {
                    if num % 10u128.pow(*pos as u32) == *tail {
                        winning_quantity += 1;
                    }
                }
            }

            // TODO: reward transfering not implemented.
            let user_infos = self.lotteries
                .get_mut(&pool_id).unwrap()
                .get_mut(&lot_times).unwrap().user_infos
                .get_mut(&caller).unwrap();
            for (token20, amount) in &reward_amount {
                let user_reward = amount.saturating_mul(winning_quantity).saturating_div(total_win_quantity);
                // let token20_instance = FromAccountId::from_account_id(*token20);
                // token20_instance.transfer(caller, user_reward);
                user_infos.reward_amount.entry(*token20)
                    .and_modify(|v| *v += user_reward)
                    .or_insert(user_reward);
            }
            user_infos.sections.entry((start, end))
                .and_modify(|v| *v = true);
        }

        fn lottery_info(
            &self, 
            pool_id: u128, 
            lot_times: u128
        ) -> (BTreeMap<u128, u8>, BTreeMap<AccountId, u128>, u128) {
            let lot = self.lotteries
                .get(&pool_id).unwrap()
                .get(&lot_times).unwrap();
            (lot.winning_tails.clone(), lot.reward_amount.clone(), lot.winning_quantity)
        }

        fn only_mb_market(&self) {
            assert_eq!(self.mystery_box_market, self.env().caller(), 
                "only mystery box market contract authorized");
        }

        fn lot_times(&self, pool_id: u128) -> u128 {
            self.lotteries.get(&pool_id).unwrap().len() as u128
        }

        fn total_quantity(&self, pool_id: u128, lot_times: u128) -> u128 {
            let _total = self.lotteries
                .get(&pool_id).unwrap()
                .get(&lot_times).unwrap().total_quantity;
            _total
        }

        /// How many times does a account buy of current lottery times in current pool.
        #[ink(message)]
        pub fn buy_times(&self, pool_id: u128, lot_times: u128, user: AccountId) -> u128 {
            let _total = self.lotteries
                .get(&pool_id).unwrap()
                .get(&lot_times).unwrap().user_infos
                .get(&user).unwrap()
                .sections.len() as u128;
            _total
        }

        fn section_info(
            &self, 
            pool_id: u128, 
            lot_times: u128, 
            user: AccountId, 
            buy_times: u128
        ) -> (u128, u128, bool) {
            let _info = self.lotteries
                .get(&pool_id).unwrap()
                .get(&lot_times).unwrap().user_infos
                .get(&user).unwrap().sections
                .iter().collect::<Vec<_>>()[buy_times as usize - 1];
            (_info.0.0, _info.0.1, *_info.1)
        }

        #[ink(message)]
        pub fn get_reward_token20s(&self) {}

        #[ink(message)]
        pub fn get_winning_data(&self, _pool_id: u128, _lot_times: u128) {}

        #[ink(message)]
        pub fn get_lottery_reward_ratio(&self, _pool_id: u128, _lot_times: u128) {}

        #[ink(message)]
        pub fn get_lottery_reward_amount(&self, _pool_id: u128, _lot_times: u128, _token20: AccountId) {}
        
        #[ink(message)]
        pub fn get_user_buy_times(&self, _pool_id: u128, _lot_times: u128, _user: AccountId) {}

        #[ink(message)]
        pub fn get_user_reward_amount(
            &self,
            _pool_id: u128,
            _lot_times: u128,
            _user: AccountId,
            _token20: AccountId,
        ) {}

        #[ink(message)]
        pub fn get_user_numbers(
            &self,
            _pool_id: u128,
            _lot_times: u128,
            _user: AccountId,
            _buy_times: u128,
        ) {}

        #[ink(message)]
        pub fn min_lottery_addr_quantity(&self, pool_id: u128) -> u128 {
            *self.min_lottery_addr_quantities.get(&pool_id).unwrap_or(&500000)
        }

        #[ink(message)]
        pub fn get_token20_balance(&self, _token20: AccountId) {}
    }

    impl Lottery {
        fn new() -> Lottery {
            Lottery {
                winning_quantity: 0,
                total_quantity: 0,
                reward_ratio: 0,
                winning_tails: BTreeMap::new(),
                reward_amount: BTreeMap::new(),
                user_infos: BTreeMap::new(),
            }
        }
    }

    impl UserInfo {
        fn new() -> UserInfo {
            UserInfo {
                buy_quantity: 0,
                sections: BTreeMap::new(),
                reward_amount: BTreeMap::new(),
            }
        }
    }

    fn modify_user_info(user_info: &mut UserInfo, total_quantity: u128, quantity: u128) {
        user_info.buy_quantity += quantity;
        user_info.sections.insert((total_quantity - quantity + 1, total_quantity), false);
    }
}
