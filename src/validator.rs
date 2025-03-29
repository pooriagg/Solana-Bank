use {
    crate::{
        error::BankError,
        state::{
            Signature,
            UserBankAccount
        }
    },
    solana_program::{
        program_error::ProgramError,
        pubkey::Pubkey,
        entrypoint::ProgramResult,
        account_info::AccountInfo,
        program_memory::{
            sol_memcmp,
            sol_memcpy
        },
        clock::Epoch
    },
    std::str::FromStr
};

/// Example-For-MessageV1 -> "<pubkey>,<lamports>,<memo>"
#[derive(Debug)]
pub struct MessageV1 {
    pub signer: Pubkey,
    pub signature: Signature,
    pub to: Pubkey,
    pub lamports: u64,
    pub memo: String
}

/// Example-For-MessageV2 -> "<pubkey>,<mint>,<amount>,<memo>"
#[derive(Debug)]
pub struct MessageV2 {
    pub signer: Pubkey,
    pub signature: Signature,
    pub to: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub memo: String
}

// constants
const WITHDRAW_WITH_ED25519_LAMPORTS_ARGS_COUNT: usize = 3;
const WITHDRAW_WITH_Ed25519__SPL_TOKEN__ARGS_COUNT: usize = 4;

/// Message validator for lamports withdraw
pub(crate) fn validate_message_v1(ed25519_signature_data: &Vec<u8>) -> Result<MessageV1, ProgramError> {
    let signer = ed25519_signature_data.get(16..48).unwrap();
    let signature = ed25519_signature_data.get(48..112).unwrap();
    let message = ed25519_signature_data.get(112..).unwrap();

    let msg = String::from_utf8(
        message
            .try_into()
            .unwrap()
    ).unwrap();
    let message_info = msg.split(",").collect::<Vec<_>>();

    if message_info.len() != WITHDRAW_WITH_ED25519_LAMPORTS_ARGS_COUNT {
        return Err(
            ProgramError::Custom(
                BankError::MessageV1ValidationFailed as u32
            )
        );
    };

    let to = Pubkey::from_str(message_info[0])
        .map_err(|_| {
            ProgramError::Custom(
                BankError::InvalidToPubkey as u32
            )
        }).unwrap();
    
    let lamports = message_info[1]
        .parse::<u64>()
        .map_err(|_| {
            ProgramError::Custom(
                BankError::InvalidLamports as u32
            )
        }).unwrap();
        
    let memo = message_info[2].to_owned();

    Ok(
        MessageV1 {
            signer: Pubkey::try_from(signer).unwrap(),
            signature: signature.try_into().unwrap(),
            to,
            lamports,
            memo
        }
    )
}

/// Message validator for spl-tokens withdraw
pub(crate) fn validate_message_v2(ed25519_signature_data: &Vec<u8>) -> Result<MessageV2, ProgramError> {
    let signer = ed25519_signature_data.get(16..48).unwrap();
    let signature = ed25519_signature_data.get(48..112).unwrap();
    let message = ed25519_signature_data.get(112..).unwrap();

    let msg = String::from_utf8(
        message
            .try_into()
            .unwrap()
    ).unwrap();
    let message_info = msg.split(",").collect::<Vec<_>>();

    if message_info.len() != WITHDRAW_WITH_Ed25519__SPL_TOKEN__ARGS_COUNT {
        return Err(
            ProgramError::Custom(
                BankError::MessageV2ValidationFailed as u32
            )
        );
    };

    let to = Pubkey::from_str(message_info[0])
        .map_err(|_| {
            ProgramError::Custom(
                BankError::InvalidToPubkey as u32
            )
        }).unwrap();

    let mint = Pubkey::from_str(message_info[1])
        .map_err(|_| {
            ProgramError::Custom(
                BankError::InvalidMint as u32
            )
        }).unwrap();

    let amount = message_info[2]
        .parse::<u64>()
        .map_err(|_| {
            ProgramError::Custom(
                BankError::InvalidTokenAmount as u32
            )
        }).unwrap();

    let memo = message_info[3].to_owned();

    Ok(
        MessageV2 {
            signer: Pubkey::try_from(signer).unwrap(),
            signature: signature.try_into().unwrap(),
            to,
            mint,
            amount,
            memo
        }
    )
}

pub(crate) fn validate_bank_account(
    program_id: &Pubkey,
    authority: &Pubkey,
    bank_account_info: &AccountInfo
) -> ProgramResult {
    if bank_account_info.owner != program_id {
        return Err(
            ProgramError::InvalidAccountOwner
        );
    };

    let bank_account_data = &bank_account_info.data.try_borrow().unwrap()[..];

    let cmp_result = sol_memcmp(
        bank_account_data.get(..8).unwrap(),
        UserBankAccount::get_bank_account_discriminator().as_slice(),
        8
    );
    if cmp_result != 0_i32 {
        return Err(
            ProgramError::InvalidAccountData
        );
    };

    let expected_bank_account_pubkey = UserBankAccount::get_user_bank_account_using_cpa(
        authority,
        bank_account_data.get(40).unwrap(), /// bump_offset
        program_id
    );
    if *bank_account_info.key != expected_bank_account_pubkey {
        return Err(
            ProgramError::InvalidSeeds
        );
    };

    Ok(())
}

#[cfg(test)]
mod test_validators {
    use std::{
        cell::RefCell,
        rc::Rc
    };
    use super::*;

    #[test]
    fn valdiate_message_v1_success() {
        let ed25519: Vec<u8> = vec![
            1,0,48,0,255,255,16,0,255,255,112,0,70,0,255,255,187,220,42,181,173,60,36,199,230,
            65,125,124,22,8,191,157,169,169,85,113,83,0,79,147,213,225,127,24,199,48,221,200,
            180,255,54,4,159,187,73,74,51,186,168,106,147,16,201,14,106,1,49,196,88,116,177,
            137,13,198,252,34,171,201,99,51,187,100,183,46,111,86,128,156,103,161,229,61,73,
            72,133,239,84,27,37,192,242,126,121,29,166,79,235,157,205,28,183,2,55,66,101,71,
            121,102,65,71,103,101,104,67,54,102,86,80,55,81,80,72,104,87,103,71,106,119,83,112,
            97,74,105,118,78,49,54,69,81,72,87,54,111,89,84,116,44,49,48,48,48,44,72,101,108,108,
            111,32,80,111,111,114,105,97,71,71,32,240,159,152,131,33
        ];

        let expected_pubkey = Pubkey::new_from_array(
            ed25519
                .get(16..48)
                .unwrap()
                .try_into()
                .unwrap()
        );
        let expected_signature: [u8; 64] = ed25519
            .get(48..112)
            .unwrap()
            .try_into()
            .unwrap();
        let expected_to = Pubkey::from_str(
            "7BeGyfAGgehC6fVP7QPHhWgGjwSpaJivN16EQHW6oYTt"
        ).unwrap();
        let expected_lamports = 1000u64;
        let expected_memo = "Hello PooriaGG 😃!".to_owned();

        let message_v1 = validate_message_v1(&ed25519).unwrap();
        
        assert_eq!(
            expected_pubkey,
            message_v1.signer,
            "expected_pubkey != message_v1.signer"
        );
        assert_eq!(
            expected_signature,
            message_v1.signature,
            "expected_signature != message_v1.siganture"
        );
        assert_eq!(
            expected_to,
            message_v1.to,
            "expected_to != message_v1.to"
        );
        assert_eq!(
            expected_lamports,
            message_v1.lamports,
            "expected_lamports != message_v1.lamports"
        );
        assert_eq!(
            expected_memo,
            message_v1.memo,
            "expected_memo != message_v1.memo"
        );
    }

    #[test]
    fn validate_message_v1_fail() {
        let ed25519: Vec<u8> = vec![
            1,0,48,0,255,255,16,0,255,255,112,0,70,0,255,255,187,220,42,181,173,60,36,199,230,
            65,125,124,22,8,191,157,169,169,85,113,83,0,79,147,213,225,127,24,199,48,221,200,
            180,255,54,4,159,187,73,74,51,186,168,106,147,16,201,14,106,1,49,196,88,116,177,
            137,13,198,252,34,171,201,99,51,187,100,183,46,111,86,128,156,103,161,229,61,73,
            72,133,239,84,27,37,192,242,126,121,29,166,79,235,157,205,28,183,2,55,66,101,71,
            121,102,65,71,103,101,104,67,54,102,86,80,55,81,80,72,104,87,103,71,106,119,83,112,
            97,74,105,118,78,49,54,69,81,72,87,54,111,89,84,116,44,49,48,48,48
        ];

        let error = validate_message_v1(&ed25519).unwrap_err();
        assert_eq!(
            ProgramError::Custom(
                BankError::MessageV1ValidationFailed as u32
            ),
            error,
            "Mismatch error types!"
        );
    }

    #[test]
    fn valdiate_message_v2_success() {
        let ed25519: Vec<u8> = vec![
            1,0,48,0,255,255,16,0,255,255,112,0,99,0,255,255,187,220,42,181,173,60,36,199,230,65,125,124,
            22,8,191,157,169,169,85,113,83,0,79,147,213,225,127,24,199,48,221,200,31,122,253,233,93,129,
            205,198,171,211,41,194,215,77,113,166,170,185,210,119,98,91,212,146,232,95,223,212,71,94,215,
            200,241,70,251,227,29,58,198,168,230,220,28,193,78,90,145,2,234,29,82,243,255,245,1,78,103,79,
            169,6,19,2,2,5,55,66,101,71,121,102,65,71,103,101,104,67,54,102,86,80,55,81,80,72,104,87,103,
            71,106,119,83,112,97,74,105,118,78,49,54,69,81,72,87,54,111,89,84,116,44,69,80,106,70,87,100,
            100,53,65,117,102,113,83,83,113,101,77,50,113,78,49,120,122,121,98,97,112,67,56,71,52,119,69,
            71,71,107,90,119,121,84,68,116,49,118,44,49,48,48,48,44,240,159,152,131
        ];

        let expected_pubkey = Pubkey::new_from_array(
            ed25519
                .get(16..48)
                .unwrap()
                .try_into()
                .unwrap()
        );
        let expected_signature: [u8; 64] = ed25519
            .get(48..112)
            .unwrap()
            .try_into()
            .unwrap();
        let expected_to = Pubkey::from_str(
            "7BeGyfAGgehC6fVP7QPHhWgGjwSpaJivN16EQHW6oYTt"
        ).unwrap();
        let expected_mint = Pubkey::from_str(
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
        ).unwrap();
        let expected_token_amount = 1000u64;
        let expected_memo = "😃".to_owned();

        let message_v2 = validate_message_v2(&ed25519).unwrap();

        assert_eq!(
            expected_pubkey,
            message_v2.signer,
            "expected_pubkey != message_v2.signer"
        );
        assert_eq!(
            expected_signature,
            message_v2.signature,
            "expected_signature != message_v2.siganture"
        );
        assert_eq!(
            expected_to,
            message_v2.to,
            "expected_to != message_v2.to"
        );
        assert_eq!(
            expected_mint,
            message_v2.mint,
            "expected_mint != message_v2.mint"
        );
        assert_eq!(
            expected_token_amount,
            message_v2.amount,
            "expected_token_amount != message_v2.amount"
        );
        assert_eq!(
            expected_memo,
            message_v2.memo,
            "expected_memo != message_v2.memo"
        );
    }

    #[test]
    fn validate_message_v2_fail() {
        let ed25519: Vec<u8> = vec![
            1,0,48,0,255,255,16,0,255,255,112,0,99,0,255,255,187,220,42,181,173,60,36,199,230,65,125,124,
            22,8,191,157,169,169,85,113,83,0,79,147,213,225,127,24,199,48,221,200,31,122,253,233,93,129,
            205,198,171,211,41,194,215,77,113,166,170,185,210,119,98,91,212,146,232,95,223,212,71,94,215,
            200,241,70,251,227,29,58,198,168,230,220,28,193,78,90,145,2,234,29,82,243,255,245,1,78,103,79,
            169,6,19,2,2,5,55,66,101,71,121,102,65,71,103,101,104,67,54,102,86,80,55,81,80,72,104,87,103,
            71,106,119,83,112,97,74,105,118,78,49,54,69,81,72,87,54,111,89,84,116,44,69,80,106,70,87,100,
            100,53,65,117,102,113,83,83,113,101,77,50,113,78,49,120,122,121,98,97,112,67,56,71,52,119,69,
            71,71,107,90,119,121,84,68,116,49,118,44,49,48,48,48
        ];

        let error = validate_message_v2(&ed25519).unwrap_err();
        assert_eq!(
            ProgramError::Custom(
                BankError::MessageV2ValidationFailed as u32
            ),
            error,
            "Mismatch error types!"
        );
    }

    #[test]
    fn validate_bank_account_success() {
        let program_id = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let bank_account = UserBankAccount::get_user_bank_account_using_fpa(&authority, &program_id);

        let mut bank_account_data: &mut [u8] = &mut [0; 41];
        bank_account_data[40] = bank_account.1;

        let dis = UserBankAccount::get_bank_account_discriminator();
        let discriminator = dis.as_slice();

        sol_memcpy(bank_account_data, discriminator, 8);

        let mut balance = solana_program::native_token::sol_to_lamports(0.5);

        let bank_account_info: AccountInfo = AccountInfo {
            key: &bank_account.0,
            lamports: Rc::new(
                RefCell::new(
                    &mut balance
                )
            ) ,
            owner: &program_id,
            rent_epoch: Epoch::default(),
            data: Rc::new(
                RefCell::new(
                    bank_account_data
                )
            ),
            is_signer: false,
            is_writable: false,
            executable: false
        };

        validate_bank_account(&program_id, &authority, &bank_account_info).unwrap();
    }

    #[test]
    fn validate_bank_account_fail_1() {
        let program_id = Pubkey::new_unique();
        let fake_program_id = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let bank_account = UserBankAccount::get_user_bank_account_using_fpa(&authority, &program_id);

        let mut bank_account_data: &mut [u8] = &mut [0; 41];
        bank_account_data[40] = bank_account.1;

        let dis = UserBankAccount::get_bank_account_discriminator();
        let discriminator = dis.as_slice();

        sol_memcpy(bank_account_data, discriminator, 8);

        let mut balance = solana_program::native_token::sol_to_lamports(0.5);

        let bank_account_info: AccountInfo = AccountInfo {
            key: &bank_account.0,
            lamports: Rc::new(
                RefCell::new(
                    &mut balance
                )
            ) ,
            owner: &fake_program_id,
            rent_epoch: Epoch::default(),
            data: Rc::new(
                RefCell::new(
                    bank_account_data
                )
            ),
            is_signer: false,
            is_writable: false,
            executable: false
        };

        let error = validate_bank_account(&program_id, &authority, &bank_account_info).unwrap_err();

        assert_eq!(
            ProgramError::InvalidAccountOwner,
            error,
            "Mismatch error types!"
        );
    }

    #[test]
    fn validate_bank_account_fail_2() {
        let program_id = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let bank_account = UserBankAccount::get_user_bank_account_using_fpa(&authority, &program_id);

        let mut bank_account_data: &mut [u8] = &mut [0; 41];
        bank_account_data[40] = bank_account.1;

        let mut balance = solana_program::native_token::sol_to_lamports(0.5);

        let bank_account_info: AccountInfo = AccountInfo {
            key: &bank_account.0,
            lamports: Rc::new(
                RefCell::new(
                    &mut balance
                )
            ) ,
            owner: &program_id,
            rent_epoch: Epoch::default(),
            data: Rc::new(
                RefCell::new(
                    bank_account_data
                )
            ),
            is_signer: false,
            is_writable: false,
            executable: false
        };

        let error = validate_bank_account(&program_id, &authority, &bank_account_info).unwrap_err();

        assert_eq!(
            ProgramError::InvalidAccountData,
            error,
            "Mismatch error types!"
        );
    }

    #[test]
    fn validate_bank_account_fail_3() {
        let program_id = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let bank_account = UserBankAccount::get_user_bank_account_using_fpa(&authority, &program_id);

        let mut bank_account_data: &mut [u8] = &mut [0; 41];
        bank_account_data[40] = 100u8;

        let dis = UserBankAccount::get_bank_account_discriminator();
        let discriminator = dis.as_slice();

        sol_memcpy(bank_account_data, discriminator, 8);

        let mut balance = solana_program::native_token::sol_to_lamports(0.5);

        let bank_account_info: AccountInfo = AccountInfo {
            key: &bank_account.0,
            lamports: Rc::new(
                RefCell::new(
                    &mut balance
                )
            ) ,
            owner: &program_id,
            rent_epoch: Epoch::default(),
            data: Rc::new(
                RefCell::new(
                    bank_account_data
                )
            ),
            is_signer: false,
            is_writable: false,
            executable: false
        };

       let error = validate_bank_account(&program_id, &authority, &bank_account_info).unwrap_err();

       assert_eq!(
           ProgramError::InvalidSeeds,
           error,
           "Mismatch error types!"
       );
    }
}
