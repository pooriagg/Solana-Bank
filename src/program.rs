use solana_program::{
    pubkey::Pubkey,
    pubkey
};

pub const PROGRAM_ID: Pubkey = pubkey!("11111111111111111111111111111111");

pub fn check_id(program_id: &Pubkey) -> bool {
    *program_id == PROGRAM_ID
}