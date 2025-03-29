use {
    borsh::{
        BorshDeserialize,
        BorshSerialize
    },
    
    solana_program::{
        program_error::ProgramError,
        pubkey::Pubkey,
        instruction::{
            Instruction,
            AccountMeta
        }
    }
};

#[derive(Debug, Clone, Copy, PartialEq, BorshDeserialize, BorshSerialize)]
pub enum BankInstruction {
    /// create new on-chain bank account
    /// 
    /// Accounts expected by this instruction:
    /// 
    ///     0. `[writable,signer]` funding account for new bank-account creation
    ///     1. `[]` authority of the newly created bank-account
    ///     2. `[writable]` new bank-account
    ///     3. `[]` system-program account 
    CreateBankAccount,
    
    /// withdraw lamports from bank-account
    /// 
    /// Accounts expected by this instruction:
    /// 
    ///     0. `[signer]` bank-account's authority account
    ///     1. `[writable]` bank-account
    ///     2. `[writable]` funds recepient account
    WithdrawLamports {
        /// lamports to withdraw from bank-account
        lamports: u64
    },
    
    /// withdraw tokens from bank-account's associated-token-account
    ///
    /// NOTE : The bank-account's A.T.A for the specific spl-token must be created and initialized before invoking this instruction (owner of the A.T.A must be the authority of the bank-account)
    /// 
    /// Accounts expected by this instruction:
    /// 
    ///     0. `[signer]` bank-account's authority account
    ///     1. `[]` bank-account
    ///     2. `[writable]` bank-account's associated-token-account
    ///     3. `[]` mint account
    ///     4. `[writable]` destination token-account
    ///     5. `[]` token program account
    WithdrawSplTokens {
        /// token-amount to withdraw from bank-account's associated-token-account
        amount: u64
    },
    
    /// withdraw lamports from bank-account's associated-token-account using ed25519 signature
    /// 
    /// previous instruction must be an ed25519-signature-verification instruction
    /// 
    /// Accounts expected by this instruction:
    /// 
    /// 0. `[writable]` bank-account
    /// 1. `[writable,signer]` funder for bank-account size increase
    /// 2. `[signer]` "to" account of the ed25519 signature
    /// 3. `[writable]` recepient account of lamports
    /// 4. `[]` system program account
    /// 5. `[]` memo program account (if memo message provided in the message)
    WithdrawLamportsUsingEd25519Signature,
    
    /// withdraw tokens from bank-account's associated-token-account using ed25519 signature
    /// 
    /// NOTE : The bank-account's A.T.A for the specific spl-token must be created and initialized before invoking this instruction (owner of the A.T.A must be the authority of the bank-account)
    ///
    /// previous instruction must be an ed25519-signature-verification instruction
    /// 
    /// Accounts expected by this instruction:
    /// 
    /// 0. `[]` mint-account
    /// 1. `[writable]` destination bank-account
    /// 2. `[writable]` source bank-account associated token-account
    /// 3. `[writable,signer]` funder for bank-account size increase
    /// 4. `[signer]` "to" account of the ed25519 signature
    /// 5. `[writable]` destination token-account
    /// 6. `[]` token standard program account
    /// 7. `[]` system program account
    /// 8. `[]` memo program account (if memo message provided in the message)
    WithdrawSplToknesUsingEd25519Signature
}

impl BankInstruction {
    pub fn unpack(instruction_data: &[u8]) -> Result<BankInstruction, ProgramError> {
        BankInstruction::try_from_slice(instruction_data)
            .map_err(|_| ProgramError::InvalidInstructionData)
    }
}

pub fn create_create_initialize_bank_account_instruction(
    funding_account: &Pubkey,
    authority_account: &Pubkey,
    solana_bank_account: &Pubkey,
    system_program_account: &Pubkey,
    program_id: &Pubkey
) -> Instruction {
    Instruction {
        program_id: *program_id,
        data: BankInstruction::CreateBankAccount.try_to_vec().unwrap(),
        accounts: vec![
            AccountMeta::new(*funding_account, true),
            AccountMeta::new_readonly(*authority_account, false),
            AccountMeta::new(*solana_bank_account, false),
            AccountMeta::new_readonly(*system_program_account, false)
        ]
    }
}

pub fn create_withdraw_lamports(
    authority_account: &Pubkey,
    bank_account: &Pubkey,
    recepient_account: &Pubkey,
    program_id: &Pubkey,
    lamports: &u64
) -> Instruction {
    Instruction {
        program_id: *program_id,
        data: BankInstruction::WithdrawLamports { lamports: *lamports }.try_to_vec().unwrap(),
        accounts: vec![
            AccountMeta::new_readonly(*authority_account, true),
            AccountMeta::new(*bank_account, false),
            AccountMeta::new(*recepient_account, false)
        ]
    }
}

pub fn create_withdraw_spl_tokens(
    authority_account: &Pubkey,
    bank_account: &Pubkey,
    bank_account_associated_token_account: &Pubkey,
    mint_account: &Pubkey,
    destination_token_account: &Pubkey,
    program_id: &Pubkey,
    amount: &u64
) -> Instruction {
    Instruction {
        program_id: *program_id,
        data: BankInstruction::WithdrawSplTokens { amount: *amount }.try_to_vec().unwrap(),
        accounts: vec![
            AccountMeta::new_readonly(*authority_account, true),
            AccountMeta::new_readonly(*bank_account, false),
            AccountMeta::new(*bank_account_associated_token_account, false),
            AccountMeta::new_readonly(*mint_account, false),
            AccountMeta::new(*destination_token_account, false),
            AccountMeta::new_readonly(spl_token::ID, false)
        ]
    }
}

pub fn create_withdraw_lamports_using_ed25519_signature(
    bank_account: &Pubkey,
    funder_account: &Pubkey,
    withdrawer_account: &Pubkey,
    recepient_account: &Pubkey,
    system_program_account: &Pubkey,
    memo_program_account: Option<&Pubkey>,
    program_id: &Pubkey
) -> Instruction {
    match memo_program_account {
        Some(memo_program_addr) => Instruction {
            program_id: *program_id,
            data: BankInstruction::WithdrawLamportsUsingEd25519Signature.try_to_vec().unwrap(),
            accounts: vec![
                AccountMeta::new(*bank_account, false),
                AccountMeta::new(*funder_account, true),
                AccountMeta::new_readonly(*withdrawer_account, true),
                AccountMeta::new(*recepient_account, false),
                AccountMeta::new_readonly(*system_program_account, false),
                AccountMeta::new_readonly(*memo_program_addr, false)
            ]
        },
        None => Instruction {
            program_id: *program_id,
            data: BankInstruction::WithdrawLamportsUsingEd25519Signature.try_to_vec().unwrap(),
            accounts: vec![
                AccountMeta::new(*bank_account, false),
                AccountMeta::new(*funder_account, true),
                AccountMeta::new_readonly(*withdrawer_account, true),
                AccountMeta::new(*recepient_account, false),
                AccountMeta::new_readonly(*system_program_account, false)
            ]
        }
    }
}

pub fn create_withdraw_spl_tokens_using_ed25519_signature(
    mint_account: &Pubkey,
    bank_account: &Pubkey,
    bank_associated_token_account: &Pubkey,
    funder_account: &Pubkey,
    withdrawer_account: &Pubkey,
    destination_token_account: &Pubkey,
    token_program_account: &Pubkey,
    system_program_account: &Pubkey,
    memo_program_account: Option<&Pubkey>,
    program_id: &Pubkey
) -> Instruction {
    match memo_program_account {
        Some(memo_program_addr) => Instruction {
            program_id: *program_id,
            data: BankInstruction::WithdrawSplToknesUsingEd25519Signature.try_to_vec().unwrap(),
            accounts: vec![
                AccountMeta::new_readonly(*mint_account, false),
                AccountMeta::new(*bank_account, false),
                AccountMeta::new(*bank_associated_token_account, false),
                AccountMeta::new(*funder_account, true),
                AccountMeta::new_readonly(*withdrawer_account, true),
                AccountMeta::new(*destination_token_account, false),
                AccountMeta::new_readonly(*token_program_account, false),
                AccountMeta::new_readonly(*system_program_account, false),
                AccountMeta::new_readonly(*memo_program_addr, false)
            ]
        },
        None => Instruction {
            program_id: *program_id,
            data: BankInstruction::WithdrawSplToknesUsingEd25519Signature.try_to_vec().unwrap(),
            accounts: vec![
                AccountMeta::new_readonly(*mint_account, false),
                AccountMeta::new(*bank_account, false),
                AccountMeta::new(*bank_associated_token_account, false),
                AccountMeta::new(*funder_account, true),
                AccountMeta::new_readonly(*withdrawer_account, true),
                AccountMeta::new(*destination_token_account, false),
                AccountMeta::new_readonly(*token_program_account, false),
                AccountMeta::new_readonly(*system_program_account, false)
            ]
        }
    }
}
