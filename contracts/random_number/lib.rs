#![cfg_attr(not(feature = "std"), no_std)]

pub use self::random_number::RandomNumber;
use ink_lang as ink;

#[ink::contract]
mod random_number {
    #[cfg(not(feature = "ink-as-dependency"))]
    use scale::{Encode, Decode}; 

    #[derive(Default)]
    #[ink(storage)]
    pub struct RandomNumber {
        random_number: u32,
    }

    impl RandomNumber {
        #[ink(constructor)]
        pub fn new() -> Self {
            Self::default()
        }

        #[ink(message)]
        pub fn random(&mut self, _salt: u32) -> u32 {
            let pre_random_number = self.random_number;
            let current_time: u64 = self.env().block_timestamp().into();
            let current_block = self.env().block_number();

            // mix the four integers whatever you like, and then encode it to a Vec<u8>.
            let mix_data = current_time as u32 ^ current_block | pre_random_number + _salt;
            let mix_seed = mix_data.encode();  // encode as a Vec<u8>

            let (hash, _) = self.env().random(&mix_seed);
            self.random_number = <BlockNumber>::decode(&mut hash.as_ref()).expect("get random number failed");
            self.random_number
        }

        #[ink(message)]
        pub fn random_number(&self) -> u32 {
            self.random_number
        }
    }
}
