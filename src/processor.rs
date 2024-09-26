use {
    crate::{
        error::BankError,
        state::{
            UserBankAccount,
            VerifiedSignature
        },
        validator::{
            validate_bank_account,
            validate_message_v1,
            validate_message_v2,
            MessageV1,
            MessageV2
        },
        instruction::BankInstruction
    },
    borsh::{
        BorshDeserialize,
        BorshSerialize
    },
    solana_program::{
        account_info::{
            next_account_info,
            AccountInfo
        },
        borsh0_10::try_from_slice_unchecked,
        entrypoint::ProgramResult,
        msg,
        program::{
            invoke_signed,
            invoke
        },
        program_error::ProgramError,
        pubkey::{
            Pubkey,
            PUBKEY_BYTES
        },
        pubkey,
        system_instruction::{
            create_account as create_solana_account,
            transfer as transfer_lamports
        },
        system_program::ID as SYSTEM_PROGRAM_ID,
        sysvar::{
            clock::Clock,
            rent::Rent,
            Sysvar
        },
        program_pack::Pack,
        instruction::{
            get_processed_sibling_instruction,
            Instruction,
            AccountMeta
        },
        ed25519_program::ID as ED25519_PROGRAM_ID
    },
    spl_token::{
        state::{
            Mint,
            Account as TokenAccount
        },
        instruction::transfer_checked as transfer_spl_token_checked,
        ID as SPL_TOKEN_PROGRAM_ID
    }
};

/// space needed for creating bank-account
pub const DISCRIMINATOR_SIZE: usize = 8;
pub const AUTHORITY_SIZE: usize = PUBKEY_BYTES;
pub const NEGATIVE_SIGNATURES_SIZE: usize = 2;
pub const BUMP_SIZE: usize = 1;
pub const CREATION_TIME_SIZE: usize = 8;
pub const SIGNATURES_SIZE: usize = 4 + 0;
//////////////////////////////////////////
pub const MEMO_PROGRAM_ID: Pubkey = pubkey!("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr");
pub const RENT_EXEMPT_YEARS_REQUIRED: u8 = 2;

pub struct Processor {}
impl Processor {
    fn process_create_initialize_bank_account(
        program_id: &Pubkey,
        accounts_info: &[AccountInfo]
    ) -> ProgramResult {
        let accounts_info = &mut accounts_info.iter();

        let funding_account_info = next_account_info(accounts_info)?;
        let authority_account_info = next_account_info(accounts_info)?;
        let solana_bank_account_info = next_account_info(accounts_info)?;
        let system_program_account_info = next_account_info(accounts_info)?;

        if solana_bank_account_info.data_len() > 0_usize {
            return Err(
                ProgramError::InvalidAccountData
            );
        };

        if *system_program_account_info.key != SYSTEM_PROGRAM_ID {
            return Err(
                ProgramError::InvalidAccountOwner
            );
        };

        let (
            bank_account_addr,
            bump
        ) = UserBankAccount::get_user_bank_account_using_fpa(
            authority_account_info.key,
            program_id
        );

        if bank_account_addr != *solana_bank_account_info.key {
            return Err(
                ProgramError::InvalidSeeds
            );
        };

        let space = 
            DISCRIMINATOR_SIZE +
            AUTHORITY_SIZE + 
            NEGATIVE_SIGNATURES_SIZE +
            BUMP_SIZE + 
            CREATION_TIME_SIZE +
            SIGNATURES_SIZE;
        let rent = Rent::get().unwrap().minimum_balance(space);
        
        invoke_signed(
            &create_solana_account(
                funding_account_info.key,
                solana_bank_account_info.key,
                rent,
                space as u64,
                program_id
            ),
            &[
                funding_account_info.clone(),
                solana_bank_account_info.clone(),
                system_program_account_info.clone()
            ],
            &[
                &[
                    b"user_bank_account",
                    authority_account_info.key.to_bytes().as_slice(),
                    &[bump]
                ]
            ]
        )?;

        msg!("new bank-account created.");

        let mut bank_account = try_from_slice_unchecked::<UserBankAccount>(
            &solana_bank_account_info
                .data
                .try_borrow()
                .unwrap()[..]
        ).unwrap();

        bank_account.discriminator = UserBankAccount::get_bank_account_discriminator();
        bank_account.authority = *authority_account_info.key;
        bank_account.bump = bump;
        bank_account.account_created_at = Clock::get().unwrap().unix_timestamp;

        bank_account.serialize(
            &mut &mut solana_bank_account_info.data.try_borrow_mut().unwrap()[..]
        ).unwrap();

        msg!("new bank-account initialized.");

        Ok(())
    }

    pub fn process_withdraw_lamports(
        program_id: &Pubkey,
        accounts_info: &[AccountInfo],
        lamports: &u64
    ) -> ProgramResult {
        let accounts_info = &mut accounts_info.iter();

        let authority_account_info = next_account_info(accounts_info)?;
        let bank_account_info = next_account_info(accounts_info)?;
        let recepient_account_info = next_account_info(accounts_info)?;

        if authority_account_info.is_signer == false {
            return Err(
                ProgramError::MissingRequiredSignature
            )
        };

        let validation_result = validate_bank_account(
            program_id,
            authority_account_info.key,
            bank_account_info
        );
        if let Err(err) = validation_result {
            return Err(err);
        };

        let space = bank_account_info.data_len();
        let rent = Rent::get().unwrap().minimum_balance(space);
        let balance = bank_account_info.lamports() - rent;
        if *lamports > balance {
            return Err(
                ProgramError::Custom(
                    BankError::InsufficientLamportBalance as u32
                )
            );
        };

        **bank_account_info.try_borrow_mut_lamports()? -= lamports;
        **recepient_account_info.try_borrow_mut_lamports()? += lamports;

        msg!("Lamports withdrawed.");

        Ok(())
    }

    /// only supports associated-token-accounts
    pub fn process_withdraw_spl_tokens(
        program_id: &Pubkey,
        accounts_info: &[AccountInfo],
        token_amount: &u64
    ) -> ProgramResult {
        let accounts_info = &mut accounts_info.iter();

        let authority_account_info = next_account_info(accounts_info)?;
        let bank_account_info = next_account_info(accounts_info)?;
        let bank_account_token_account_info = next_account_info(accounts_info)?;
        let mint_account_info = next_account_info(accounts_info)?;
        let destination_token_account_info = next_account_info(accounts_info)?;
        let token_program_account_info = next_account_info(accounts_info)?;

        if authority_account_info.is_signer == false {
            return Err(
                ProgramError::MissingRequiredSignature
            );
        };

        let validation_result = validate_bank_account(
            program_id,
            authority_account_info.key,
            bank_account_info
        );
        if let Err(err) = validation_result {
            return Err(err);
        };

        let expected_bank_account_token_account = Self::_get_associated_token_account(
            bank_account_info.key,
            &spl_token::id(),
            mint_account_info.key
        );
        if *bank_account_token_account_info.key != expected_bank_account_token_account {
            return Err(
                ProgramError::InvalidSeeds
            );
        };

        let bank_account_data = &bank_account_info.data.try_borrow().unwrap()[..];
        let mint_account_data = &mint_account_info.data.try_borrow().unwrap()[..];

        let decimals = Mint::unpack(
            mint_account_data
        ).unwrap().decimals;

        invoke_signed(
            &transfer_spl_token_checked(
                token_program_account_info.key,
                bank_account_token_account_info.key,
                mint_account_info.key,
                destination_token_account_info.key,
                bank_account_info.key,
                &[],
                *token_amount,
                decimals
            ).unwrap(),
            &[
                bank_account_token_account_info.clone(),
                mint_account_info.clone(),
                destination_token_account_info.clone(),
                bank_account_info.clone()
            ],
            &[
                &[
                    b"user_bank_account",
                    authority_account_info.key.to_bytes().as_slice(),
                    &[
                        *bank_account_data.get(40).unwrap() // bump_offset
                    ]
                ]
            ]
        )?;

        msg!("Tokens withdrawed.");

        Ok(())
    }

    pub fn process_withdraw_lamports_using_ed25519_signature(
        program_id: &Pubkey,
        accounts_info: &[AccountInfo]
    ) -> ProgramResult {
        let previous_instruction = get_processed_sibling_instruction(0)
            .ok_or(
                ProgramError::Custom(
                    BankError::FailedToGetEd25519Instruction as u32
                )
            );
        let ed25519_svi = match previous_instruction {
            Err(error) => return Err(error),
            Ok(ix) => ix
        };

        if ed25519_svi.program_id != ED25519_PROGRAM_ID {
            return Err(
                ProgramError::Custom(
                    BankError::InvalidEd25519SignatureVerificationInstruction as u32
                )
            );
        };

        let ed25519_data = ed25519_svi.data;
        let message_v1 = validate_message_v1(&ed25519_data);
        let MessageV1 {
            signer,
            signature,
            to,
            lamports,
            memo
        } = match message_v1 {
            Err(err) => return Err(err),
            Ok(msg_v1) => msg_v1
        };

        let accounts_info = &mut accounts_info.iter();

        let bank_account_info = next_account_info(accounts_info)?;
        let fund_account_info = next_account_info(accounts_info)?;
        let withdrawer_account_info = next_account_info(accounts_info)?;
        let recepient_account_info = next_account_info(accounts_info)?;
        let system_program_account_info = next_account_info(accounts_info)?;

        if to != *withdrawer_account_info.key {
            return Err(
                ProgramError::Custom(
                    BankError::InvalidToPubkey as u32
                )
            );
        };

        if *system_program_account_info.key != SYSTEM_PROGRAM_ID {
            return Err(
                ProgramError::Custom(
                    BankError::InvalidSystemProgramAccount as u32
                )
            );
        };

        if withdrawer_account_info.is_signer == false {
            return Err(
                ProgramError::MissingRequiredSignature
            );
        };

        let validation_result = validate_bank_account(
            program_id,
            &signer,
            bank_account_info
        );
        if let Err(err) = validation_result {
            return Err(err);
        };

        let bank_account_data = bank_account_info
            .data
            .try_borrow()
            .unwrap();
        let mut bank_account = try_from_slice_unchecked::<UserBankAccount>(
            &bank_account_data
        ).unwrap();

        let bank_account_data_size = bank_account_data.len();
        let bank_account_balance = bank_account_info.lamports() - Rent::get().unwrap().minimum_balance(bank_account_data_size);
        let is_ok: bool = if lamports > bank_account_balance {
            false
        } else {
            true
        };

        let sig_info = VerifiedSignature {
            signature,
            is_ok,
            message: ed25519_data
                .get(112..)
                .unwrap()
                .to_vec(),
            time: Clock::get().unwrap().unix_timestamp
        };

        if let Err(err) = bank_account.add_signature(&sig_info) {
            return Err(err);
        };

        let space_to_add = sig_info
            .try_to_vec()
            .unwrap()
            .len();
        let rent_for_space_increase = (
            Rent::get().unwrap().lamports_per_byte_year * (space_to_add as u64)
        ) * RENT_EXEMPT_YEARS_REQUIRED as u64;

        drop(bank_account_data);

        invoke(
            &transfer_lamports(
                fund_account_info.key,
                bank_account_info.key,
                rent_for_space_increase
            ),
            &[
                fund_account_info.clone(),
                bank_account_info.clone(),
                system_program_account_info.clone()
            ]
        )?;

        let bank_acc = bank_account_info.clone();
        let current_size = bank_acc.data_len();

        drop(bank_acc);

        bank_account_info.realloc(
            current_size + space_to_add,
            false
        ).unwrap();

        if is_ok == false {
            msg!("Insufficient lamport balance!");
            
            bank_account.serialize(
                &mut &mut bank_account_info
                    .data
                    .try_borrow_mut()
                    .unwrap()[..]
            ).unwrap();

            return Ok(());
        };

        if memo.len() > 0 {
            let memo_program_account_info = next_account_info(accounts_info)?;

            let memo_result = Self::_invoke_memo_program(
                memo_program_account_info,
                withdrawer_account_info,
                memo.as_bytes().to_vec()
            );

            if let Err(err) = memo_result {
                return Err(err);
            };
        };

        **bank_account_info.try_borrow_mut_lamports()? -= lamports;
        **recepient_account_info.try_borrow_mut_lamports()? += lamports;

        bank_account.serialize(
            &mut &mut bank_account_info
                .data
                .try_borrow_mut()
                .unwrap()[..]
        ).unwrap();

        msg!("Withdraw compeleted. v1");

        Ok(())
    }

    pub fn process_withdraw_spl_tokens_using_ed25519_signature(
        program_id: &Pubkey,
        accounts_info: &[AccountInfo]
    ) -> ProgramResult {
        let previous_instruction = get_processed_sibling_instruction(0)
            .ok_or(
                ProgramError::Custom(
                    BankError::FailedToGetEd25519Instruction as u32
                )
            );
        let ed25519_svi = match previous_instruction {
            Err(error) => return Err(error),
            Ok(ix) => ix
        };

        if ed25519_svi.program_id != ED25519_PROGRAM_ID {
            return Err(
                ProgramError::Custom(
                    BankError::InvalidEd25519SignatureVerificationInstruction as u32
                )
            );
        };

        let ed25519_data = ed25519_svi.data;
        let message_v2 = validate_message_v2(&ed25519_data);
        let MessageV2 {
            signer,
            signature,
            to,
            amount,
            memo,
            mint
        } = match message_v2 {
            Err(err) => return Err(err),
            Ok(msg_v2) => msg_v2
        };

        let accounts_info = &mut accounts_info.iter();

        let mint_account_account = next_account_info(accounts_info)?;
        let bank_account_info = next_account_info(accounts_info)?;
        let bank_assocoiated_token_account_info = next_account_info(accounts_info)?;
        let fund_account_info = next_account_info(accounts_info)?;
        let withdrawer_account_info = next_account_info(accounts_info)?;
        let destination_token_account_info = next_account_info(accounts_info)?;
        let token_standard_program_account_info = next_account_info(accounts_info)?;
        let system_program_account_info = next_account_info(accounts_info)?;

        if to != *withdrawer_account_info.key {
            return Err(
                ProgramError::Custom(
                    BankError::InvalidToPubkey as u32
                )
            );
        };

        if *system_program_account_info.key != SYSTEM_PROGRAM_ID {
            return Err(
                ProgramError::Custom(
                    BankError::InvalidSystemProgramAccount as u32
                )
            );
        };

        if *token_standard_program_account_info.key != SPL_TOKEN_PROGRAM_ID {
            return Err(
                ProgramError::Custom(
                    BankError::InvalidSplTokenProgramAccount as u32
                )
            );
        };

        if withdrawer_account_info.is_signer == false {
            return Err(
                ProgramError::MissingRequiredSignature
            );
        };

        let validation_result = validate_bank_account(
            program_id,
            &signer,
            bank_account_info
        );
        if let Err(err) = validation_result {
            return Err(err);
        };

        if *mint_account_account.key != mint {
            return Err(
                ProgramError::Custom(
                    BankError::InvalidMintAccount as u32
                )
            );
        };

        let expected_bank_associated_token_account = Self::_get_associated_token_account(
            bank_account_info.key,
            token_standard_program_account_info.key,
            mint_account_account.key
        );
        if expected_bank_associated_token_account != *bank_assocoiated_token_account_info.key {
            return Err(
                ProgramError::Custom(
                    BankError::InvalidBankAssociatedTokenAccount as u32
                )
            );
        };

        let bank_token_account = TokenAccount::unpack(
        &bank_assocoiated_token_account_info
                .data
                .try_borrow()
                .unwrap()[..]
        ).unwrap();

        let is_ok: bool = if amount > bank_token_account.amount {
            false
        } else {
            true
        };

        let signature_info = VerifiedSignature {
            is_ok,
            time: Clock::get().unwrap().unix_timestamp,
            signature,
            message: ed25519_data
                .get(112..)
                .unwrap()
                .to_vec()
        };

        let mut bank_account = try_from_slice_unchecked::<UserBankAccount>(
            &bank_account_info
                   .data
                   .try_borrow()
                   .unwrap()[..]
        ).unwrap();

        if let Err(error) = bank_account.add_signature(&signature_info) {
            return Err(error);
        };

        let space_to_add = signature_info
            .try_to_vec()
            .unwrap()
            .len();
        let rent_for_space_increase = (
            (space_to_add as u64) * Rent::get().unwrap().lamports_per_byte_year
        ) * RENT_EXEMPT_YEARS_REQUIRED as u64;

        invoke(
            &transfer_lamports(
                fund_account_info.key,
                bank_account_info.key,
                rent_for_space_increase
            ),
            &[
                fund_account_info.clone(),
                bank_account_info.clone(),
                system_program_account_info.clone()
            ]
        )?;

        let bank_acc = bank_account_info.clone();
        let current_size = bank_acc.data_len();

        drop(bank_acc);

        bank_account_info.realloc(
            current_size + space_to_add,
            false
        ).unwrap();

        if is_ok == false {
            msg!("Insufficient token balance.");
            
            bank_account.serialize(
                &mut &mut bank_account_info
                    .data
                    .try_borrow_mut()
                    .unwrap()[..]
            ).unwrap();

            return Ok(());
        };

        bank_account.serialize(
            &mut &mut bank_account_info
                .data
                .try_borrow_mut()
                .unwrap()[..]
        ).unwrap();

        let decimals = Mint::unpack(
            &mint_account_account
                .data
                .try_borrow()
                .unwrap()[..]            
        ).unwrap().decimals;

        invoke_signed(
            &transfer_spl_token_checked(
                token_standard_program_account_info.key,
                bank_assocoiated_token_account_info.key,
                mint_account_account.key,
                destination_token_account_info.key,
                bank_account_info.key,
                &[],
                amount,
                decimals
            ).unwrap(),
            &[
                bank_assocoiated_token_account_info.clone(),
                mint_account_account.clone(),
                destination_token_account_info.clone(),
                bank_account_info.clone()
            ],
            &[
                &[
                    b"user_bank_account",
                    signer.as_ref(),
                    &[bank_account.bump]
                ]
            ]
        )?;

        if memo.len() > 0 {
            let memo_program_account_info = next_account_info(accounts_info)?;

            let memo_result = Self::_invoke_memo_program(
                memo_program_account_info,
                withdrawer_account_info,
                memo.as_bytes().to_vec()
            );

            if let Err(err) = memo_result {
                return Err(err);
            };
        };

        msg!("Withdraw compeleted. v2");

        Ok(())
    }

    pub fn processor(
        program_id: &Pubkey,
        accounts_info: &[AccountInfo],
        instruction_data: &[u8]
    ) -> ProgramResult {
        let bank_instruction = BankInstruction::unpack(instruction_data).unwrap();

        match bank_instruction {
            BankInstruction::CreateBankAccount => {
                msg!("Instruction: CreateBankAccount");
                Self::process_create_initialize_bank_account(program_id, accounts_info)
            },
            BankInstruction::WithdrawLamports { lamports } => {
                msg!("Instruction: WithdrawLamports");
                Self::process_withdraw_lamports(program_id, accounts_info, &lamports)
            },
            BankInstruction::WithdrawSplTokens { amount } => {
                msg!("Instruction: WithdrawSplTokens");
                Self::process_withdraw_spl_tokens(program_id, accounts_info, &amount)
            },
            BankInstruction::WithdrawLamportsUsingEd25519Signature => {
                msg!("Instruction: WithdrawLamportsUsingEd25519Signature");
                Self::process_withdraw_lamports_using_ed25519_signature(program_id, accounts_info)
            },
            BankInstruction::WithdrawSplToknesUsingEd25519Signature => {
                msg!("Instruction: WithdrawSplToknesUsingEd25519Signature");
                Self::process_withdraw_spl_tokens_using_ed25519_signature(program_id, accounts_info)
            }
        }
    }

    fn _invoke_memo_program(
        memo_program_account_info: &AccountInfo,
        message_sender_account_info: &AccountInfo,
        memo_message: Vec<u8>
    ) -> ProgramResult {
        if *memo_program_account_info.key != MEMO_PROGRAM_ID {
            return Err(
                ProgramError::Custom(
                    BankError::InvalidMemoProgramAccount as u32
                )
            );
        };

        invoke(
            &Instruction {
                program_id: *memo_program_account_info.key,
                data: memo_message,
                accounts: vec![
                    AccountMeta::new_readonly(*message_sender_account_info.key, true)
                ]
            },
            &[
                message_sender_account_info.clone()
            ]
        )?;

        Ok(())
    }

    fn _get_associated_token_account(
        wallet_owner: &Pubkey,
        token_program_id: &Pubkey,
        mint_account: &Pubkey
    ) -> Pubkey {
        Pubkey::try_find_program_address(
            &[
                wallet_owner.to_bytes().as_slice(),
                token_program_id.to_bytes().as_slice(),
                mint_account.to_bytes().as_slice()
            ],
            &spl_associated_token_account::id()
        ).unwrap().0
    }
}