use {
    solana_program::{
        pubkey::Pubkey,
        hash::hash,
        program_error::ProgramError,
        entrypoint::ProgramResult
    },
    borsh::{
        BorshDeserialize,
        BorshSerialize
    },
    crate::{
        program::PROGRAM_ID,
        error::BankError
    }
};

pub(crate) type PdaAddress = Pubkey;
pub(crate) type Bump = u8;
pub(crate) type Signature = [u8; 64];
pub(crate) type Message = String; // utf-8 string

#[derive(Debug, PartialEq, Clone, BorshDeserialize, BorshSerialize, Default)]
pub struct UserBankAccount {
    /// discriminator
    pub discriminator: [u8; 8],
    /// authority of the on-chain bank-account
    pub authority: Pubkey,
    /// bump of the solana-bank's PDA
    pub bump: u8,
    /// time of account creation
    pub account_created_at: i64,
    /// signatures that bank-account's owner issued and beign used
    pub signatures: Vec<VerifiedSignature>
}

#[derive(Debug, BorshDeserialize, BorshSerialize, Clone, PartialEq)]
pub struct VerifiedSignature {
    /// redeemed signature
    pub signature: [u8; 64],
    /// funds was sufficient or insufficient for this signature
    pub is_ok: bool,
    /// signature activation time
    pub time: i64,
    /// siganture's message section
    pub message: Vec<u8>
}

impl UserBankAccount {
    pub fn validate_owner(
        &self,
        expected_owner: &Pubkey
    ) -> bool {
        self.authority == *expected_owner
    }

    pub fn add_signature(
        &mut self,
        signature_info: &VerifiedSignature
    ) -> ProgramResult  {
        let mut is_exist: bool = false;
        for sig_info in self.signatures.iter() {
            if sig_info.signature == signature_info.signature && sig_info.is_ok == true {
                is_exist = true;
                break;
            };
        };

        if is_exist == true {
            return Err(
                ProgramError::Custom(
                    BankError::SignatureAlreadyUsed as u32
                )
            );
        };

        self.signatures.push(
            signature_info.clone()
        );

        Ok(())
    }

    pub fn get_user_bank_account_using_fpa(
        user: &Pubkey,
        program_id: &Pubkey
    ) -> (PdaAddress, Bump) {
        Pubkey::try_find_program_address(
            &[
                b"user_bank_account",
                user.to_bytes().as_slice()
            ],
            program_id
        ).unwrap()
    }
    
    pub fn get_user_bank_account_using_cpa(
        user: &Pubkey,
        bump: &u8,
        program_id: &Pubkey
    ) -> PdaAddress {
        Pubkey::create_program_address(
            &[
                b"user_bank_account",
                user.to_bytes().as_slice(),
                &[*bump]
            ],
            program_id
        ).unwrap()
    }
    
    pub fn get_bank_account_discriminator() -> [u8; 8] {
        hash(b"account:bank_account")
            .as_ref()
            .get(..8)
            .and_then(|slice| slice.try_into().ok())
            .map(|dis: [u8; 8]| dis)
            .unwrap()
    }
}