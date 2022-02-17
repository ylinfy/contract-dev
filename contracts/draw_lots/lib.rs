#![cfg_attr(not(feature = "std"), no_std)]

pub use self::draw_lots::DrawLots;
use ink_lang as ink;

#[ink::contract]
mod draw_lots {
    use random_number::RandomNumber;
    use ink_env::call::FromAccountId;
    use ink_storage::lazy::Lazy;
    use ink_prelude::{
        vec, 
        vec::Vec,
        collections::{BTreeMap, btree_map::Entry},
    };

    #[ink(storage)]
    #[derive(Default)]
    pub struct DrawLots {
        /// The random contract
        random: Lazy<RandomNumber>,
        /// Number of digits
        highest_pos: u8,
        /// The current number of winning lots according winning tail.
        total_win_quantity: u128,
        /// Mapping from winning tail to pos
        winning_tails: BTreeMap<u128, u8>,
    }

    impl DrawLots {
        /// Constructor that initializes the `bool` value to the given `init_value`.
        #[ink(constructor)]
        pub fn new(rand_account: AccountId) -> Self {
            let random: RandomNumber = FromAccountId::from_account_id(rand_account);
            Self {
                random: Lazy::new(random),
                highest_pos: 0,
                total_win_quantity: 0,
                winning_tails: BTreeMap::new(),
            }
        }

        #[ink(message)]
        pub fn draw_lots(
            &mut self,
            salt: u32,
            target_quantity: u128,
            total_quantity: u128
        ) -> (BTreeMap<u128, u8>, bool) {
            assert!(
                target_quantity > 0 && target_quantity < total_quantity, 
                "target quantity must greater than total quantity"
            );
            self.reset_data();
            self.highest_pos = get_digits_length(total_quantity);
            let (mut _flag, mut _target, _pos) = (true, target_quantity, self.highest_pos);

            // get win rate, if win rate is greater than 50%, change to calculate not winning tails
            let mut _win_rate = _target * 10u128.pow(_pos as u32) / total_quantity;
            if _win_rate > 5 * 10u128.pow(_pos as u32 - 1) {
                _win_rate = 10u128.pow(_pos as u32) - _win_rate;
                _target = total_quantity - _target;
                _flag = false;
            }

            let mut rate_vec = vec![0; _pos as usize];
            let mut _factors = vec![0, 0];
            for i in 0.._pos {
                if rate_vec[i as usize - 1] == 0 { 
                    rate_vec[i as usize - 1] = (_win_rate as u8 / 10u8.pow(_pos as u32 - 1)) % 10;
                }
                _factors = get_factors(rate_vec[i as usize - 1]);
                self.get_winning_tail_count_quantity(
                    salt, _factors[0], i,  _target, total_quantity, &mut rate_vec);
                self.get_winning_tail_count_quantity(
                    salt, _factors[1], i, _target, total_quantity, &mut rate_vec);
            }

            // replenish lack tails in the end
            if self.total_win_quantity < _target {
                let lack_quantity = _target - self.total_win_quantity;
                self.replenish_with_highest_pos_tail(salt, lack_quantity, total_quantity);
            }

            (self.winning_tails.clone(), _flag)
        }

        fn reset_data(&mut self) {
            self.highest_pos = 0;
            self.total_win_quantity = 0;
            self.winning_tails = BTreeMap::new();
        }

        fn get_valid_winning_tail(&mut self, salt: u32, pos: u8, total_quantity: u128) -> u128 {
            let (mut flag, mut winning_tail) = (true, 0u128);
            loop {
                if flag == false { break }
                flag = false;

                // get winning tail through random number
                let rand_num = self.random.random(salt);
                winning_tail = rand_num as u128 % 10u128.pow(pos as u32);

                // if winning tail greater than total quantity, restart loop
                if winning_tail > total_quantity {
                    flag = true;
                    continue;
                }

                // compare the winning tail with each tail in self.winning_tails according the condition
                // if it turns the condition is true, then restart loop again.
                let mut _tmp_tail = 0u128;
                for n in 1..pos + 1 {
                    _tmp_tail = winning_tail % 10u128.pow(n as u32);
                    match self.winning_tails.entry(_tmp_tail) {
                        // not exist, keep going until all corresponding tails checked.
                        Entry::Vacant(_) => continue,
                        // exist, invalid number, random again
                        Entry::Occupied(_) => {flag = true; break;}
                    }
                }
            }
            winning_tail
        }

        fn storage_winning_info(&mut self, tail: u128, pos: u8) {
            self.winning_tails.insert(tail, pos);
        }

        // why replenish with highest pos tail, because it is controllable, one lot for one tail.
        fn replenish_with_highest_pos_tail(
            &mut self, 
            salt: u32, 
            lack_quantity: u128, 
            total_quantity: u128
        ) {
            let mut _winning_tail = 0u128;
            for _ in 0..lack_quantity {
                _winning_tail = self.get_valid_winning_tail(salt, self.highest_pos, total_quantity);
                self.total_win_quantity += 1;  // one lot for one tail.
                self.storage_winning_info(_winning_tail, self.highest_pos);
            } 
        }

        fn count_quantity_deal_with_win_rate(
            &mut self,
            pos: u8,
            winning_tail: u128,
            target_quantity: u128,
            total_quantity: u128,
            rate_vec: &mut Vec<u8>,
        ) {
            let quantity_per_tail = get_quantity_per_tail(pos, winning_tail, total_quantity);
            self.total_win_quantity += quantity_per_tail;  // count all lots with tails
            if self.total_win_quantity > target_quantity {
                // if true, revoke adding quantity of lots with the tail
                self.total_win_quantity -= quantity_per_tail;

                // if pos is equal to the highest, just add lack tails in the end of fn draw_lots
                // eg: 0.2800 -> 0.2799, pos is 2
                if pos != self.highest_pos {
                    rate_vec[pos as usize - 1] -= 1;
                    for i in pos..self.highest_pos {
                        rate_vec[i as usize] = 9;
                    }
                }
            } else {
                self.storage_winning_info(winning_tail, pos);
            }
        }

        fn get_winning_tail_count_quantity(
            &mut self,
            salt: u32,
            factor: u8,
            pos: u8,
            target_quantity: u128,
            total_quantity: u128,
            rate_vec: &mut Vec<u8>,
        ) {
            if factor > 0 {
                let _winning_tail = self.get_valid_winning_tail(salt, pos, total_quantity);
                self.count_quantity_deal_with_win_rate(
                    pos, _winning_tail, target_quantity, total_quantity, rate_vec);
                
                // deal with the rest 
                let step = 10 / factor;
                for mut i in 1..factor {
                    if factor == 8 && i == 4 {
                        // to make the tails even(01234567 => 01235678)
                        // the two tail discarded span 5
                        i = 8;
                    }
                    self.deal_with_rest_by_step(
                        step as u128, pos as u32, i as u128, _winning_tail, 
                        target_quantity, total_quantity, rate_vec); 
                }
            }
        }

        fn deal_with_rest_by_step(
            &mut self,
            step: u128,
            pos: u32,
            i: u128,
            winning_tail: u128,
            target_quantity: u128,
            total_quantity: u128,
            rate_vec: &mut Vec<u8>,
        ) {
            let tail_by_step = winning_tail + step * i * 10u128.pow(pos - 1);
            let mut tmp_winning_tail = if tail_by_step < 10u128.pow(pos)  {
                tail_by_step
            } else {
                tail_by_step - 10u128.pow(pos)
            };

            // when win rate bit is 7, 5+2, the last tail will be the same with one of the 5 ahead.
            // To deal with it, make the tail to span a step.
            match self.winning_tails.entry(tmp_winning_tail) {
                Entry::Vacant(_) => (),  // do nothing
                Entry::Occupied(_) => {
                    tmp_winning_tail += 10u128.pow(pos - 1);
                    if tmp_winning_tail > 10u128.pow(pos) {
                        tmp_winning_tail -= 10u128.pow(pos);
                    }
                }
            }

            if tmp_winning_tail <= total_quantity {
                self.count_quantity_deal_with_win_rate(
                    pos as u8, tmp_winning_tail, target_quantity, total_quantity, rate_vec);
            }
        }
    }

    fn get_digits_length(digit: u128) -> u8 {
        let (mut _digit, mut _length) = (digit, 0);
        
        loop {
            if _digit == 0 { break }
            _length += 1;
            _digit /= 10;
        }
        assert!(_length > 0, "Invalid digit, the digit is 0");
        _length
    }

    fn get_factors(rate_bit: u8) -> Vec<u8> {
        let mut _factors: Vec<u8> = vec![0, 0];
        match rate_bit{
            3 => { _factors = vec![2, 1] },
            4 => { _factors = vec![2, 2] },
            6 => { _factors = vec![5, 1] },
            7 => { _factors = vec![5, 2] },
            _ => { _factors[0] = rate_bit},
        }
        _factors
    }

    fn get_quantity_per_tail(pos: u8, winning_tail: u128, total_quantity: u128) -> u128 {
        let mut quantity_per_tail = total_quantity / 10u128.pow(pos as u32);
        let pos_value = total_quantity % 10u128.pow(pos as u32);

        // if the winning tail is not zero and less than pos_value, 
        // quantity for this tail should plus one
        if winning_tail != 0 && winning_tail <= pos_value {
            quantity_per_tail += 1;
        } 
        quantity_per_tail
    }
}
