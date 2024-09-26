#![allow(warnings)]

use {
    borsh::{
        BorshDeserialize,
        BorshSerialize
    },
    solana_bank::{
        error::BankError,
        instruction::*,
        processor::{
            Processor,
            MEMO_PROGRAM_ID
        },
        state::{
            UserBankAccount,
            VerifiedSignature
        }
    },
    solana_program_test::{
        processor,
        tokio,
        BanksClient,
        ProgramTest,
        ProgramTestBanksClientExt
    },
    solana_sdk::{
        account::Account as SolanaAccount,
        borsh0_10::try_from_slice_unchecked,
        clock::{
            Epoch,
            Clock
        },
        hash::Hash,
        instruction::{
            Instruction,
            InstructionError
        },
        native_token::{
            sol_to_lamports, LAMPORTS_PER_SOL
        },
        program_error,
        program_option::COption,
        program_pack::Pack,
        pubkey::Pubkey,
        rent::Rent,
        signature::Signer,
        signer::keypair::Keypair,
        system_instruction::{
            create_account as create_solana_account,
            transfer as transfer_lamports
        },
        system_program::ID as SYSTEM_PROGRAM_ID,
        transaction::{
            Transaction,
            TransactionError
        }
    },
    spl_associated_token_account::{
        instruction::create_associated_token_account,
        ID as ASSOCIATED_TOKEN_PROGRAM
    },
    spl_token::{
        instruction::{
            initialize_account3,
            initialize_mint2
        },
        state::{
            Account as TokenAccount,
            Mint
        },
        ID as TOKEN_STANDARD_PROGRAM
    }
};

fn setup(program_id: &Pubkey) -> ProgramTest {
    let program_test = ProgramTest::new(
        "solana_bank",
        *program_id,
        processor!(Processor::processor)
    );

    program_test
}

fn setup_new_mint_account(
    pt: &mut ProgramTest,
    token_program_id: &Pubkey,
    mint_account: &Pubkey,
    mint_authority: &Pubkey,
    supply: u64
) {
    let mut mint = Mint::unpack_unchecked(
        [0u8; Mint::LEN].as_slice()
    ).unwrap();
    mint.decimals = 2u8;
    mint.freeze_authority = COption::None;
    mint.mint_authority = COption::Some(*mint_authority);
    mint.supply = supply;
    mint.is_initialized = true;
    
    let mint_data: &mut [u8] = &mut [0u8; Mint::LEN];
    Mint::pack(mint, mint_data).unwrap();
    
    let solana_account_for_mint = SolanaAccount {
        owner: *token_program_id,
        lamports: Rent::default().minimum_balance(Mint::LEN),
        data: mint_data.to_vec(),
        executable: false,
        rent_epoch: Epoch::default()
    };

    pt.add_account(
        *mint_account,
        solana_account_for_mint
    );
}

fn setup_new_token_account(
    pt: &mut ProgramTest,
    token_program_id: &Pubkey,
    mint_account: &Pubkey,
    token_account_addr: &Pubkey,
    owner: &Pubkey,
    amount: u64
) {
    let mut token_account = TokenAccount::unpack_unchecked(
        [0u8; TokenAccount::LEN].as_slice()
    ).unwrap();
    token_account.close_authority = COption::Some(*owner);
    token_account.delegate = COption::None;
    token_account.delegated_amount = 0u64;
    token_account.is_native = COption::None;
    token_account.mint = *mint_account;
    token_account.owner = *owner;
    token_account.state = spl_token::state::AccountState::Initialized;
    token_account.amount = amount;
    
    let token_account_data: &mut [u8] = &mut [0u8; TokenAccount::LEN];
    TokenAccount::pack(token_account, token_account_data).unwrap();

    let solana_account_for_token_account = SolanaAccount {
        lamports: Rent::default().minimum_balance(TokenAccount::LEN),
        data: token_account_data.to_vec(),
        owner: *token_program_id,
        executable: false,
        rent_epoch: Epoch::default()
    };

    pt.add_account(
        *token_account_addr,
        solana_account_for_token_account
    );
}

fn setup_new_associated_token_account(
    pt: &mut ProgramTest,
    token_program_id: &Pubkey,
    mint_account: &Pubkey,
    token_account_owner: &Pubkey,
    amount: u64
) {
    let mut token_account = TokenAccount::unpack_unchecked(
        [0u8; TokenAccount::LEN].as_slice()
    ).unwrap();
    token_account.close_authority = COption::Some(*token_account_owner);
    token_account.delegate = COption::None;
    token_account.delegated_amount = 0u64;
    token_account.is_native = COption::None;
    token_account.mint = *mint_account;
    token_account.owner = *token_account_owner;
    token_account.state = spl_token::state::AccountState::Initialized;
    token_account.amount = amount;
    
    let token_account_data: &mut [u8] = &mut [0u8; TokenAccount::LEN];
    TokenAccount::pack(token_account, token_account_data).unwrap();

    let solana_account_for_token_account = SolanaAccount {
        lamports: Rent::default().minimum_balance(TokenAccount::LEN),
        data: token_account_data.to_vec(),
        owner: *token_program_id,
        executable: false,
        rent_epoch: Epoch::default()
    };

    let token_account_addr = Pubkey::try_find_program_address(
        &[
            token_account_owner.as_ref(),
            token_program_id.as_ref(),
            mint_account.as_ref()
        ],
        &ASSOCIATED_TOKEN_PROGRAM
    ).unwrap().0;

    pt.add_account(
        token_account_addr,
        solana_account_for_token_account
    );
}

async fn setup_new_bank_account(
    banks_client: &mut BanksClient,
    bank_account_owner: &Keypair,
    program_id: &Pubkey,
    recent_blockhash: Hash
) {
    let bank_account_pda = Pubkey::try_find_program_address(
        &[
            b"user_bank_account",
            bank_account_owner.pubkey().as_ref()
        ],
        program_id
    ).unwrap().0;

    let ix = create_create_initialize_bank_account_instruction(
        &bank_account_owner.pubkey(),
        &bank_account_owner.pubkey(),
        &bank_account_pda,
        &SYSTEM_PROGRAM_ID,
        program_id
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&bank_account_owner.pubkey()),
        &[bank_account_owner],
        recent_blockhash
    );

    banks_client
        .process_transaction(tx)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_create_bank_account_success() {
    let program_id = Pubkey::new_from_array([5; 32]);
    let pt = setup(&program_id);
    let (
        mut banks_client,
        payer,
        recent_blockhash
    ) = pt.start().await;

    let bank_account_owner = payer;
    let bank_account_pda = Pubkey::try_find_program_address(
        &[
            b"user_bank_account",
            bank_account_owner.pubkey().as_ref()
        ],
        &program_id
    ).unwrap().0;

    let ix = create_create_initialize_bank_account_instruction(
        &bank_account_owner.pubkey(),
        &bank_account_owner.pubkey(),
        &bank_account_pda,
        &SYSTEM_PROGRAM_ID,
        &program_id
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&bank_account_owner.pubkey()),
        &[&bank_account_owner],
        recent_blockhash
    );

    banks_client
        .process_transaction(tx)
        .await
        .unwrap();

    let bank_account_data = banks_client
        .get_account(bank_account_pda)
        .await
        .unwrap()
        .unwrap()
        .data;
    let bank_account_info = try_from_slice_unchecked::<UserBankAccount>(
        &bank_account_data
    ).unwrap();

    assert_eq!(
        bank_account_info.authority,
        bank_account_owner.pubkey(),
        "Authority mismatch."
    );
}

#[tokio::test]
async fn test_create_bank_account_fail_invalid_seeds() {
    let program_id = Pubkey::new_from_array([5; 32]);
    let pt = setup(&program_id);
    let (
        mut banks_client,
        payer,
        recent_blockhash
    ) = pt.start().await;

    let bank_account_owner = payer;
    let bank_account_pda = Pubkey::try_find_program_address(
        &[
            b"User_bank_account",
            bank_account_owner.pubkey().as_ref()
        ],
        &program_id
    ).unwrap().0;

    let ix = create_create_initialize_bank_account_instruction(
        &bank_account_owner.pubkey(),
        &bank_account_owner.pubkey(),
        &bank_account_pda,
        &SYSTEM_PROGRAM_ID,
        &program_id
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&bank_account_owner.pubkey()),
        &[&bank_account_owner],
        recent_blockhash
    );

    let error = banks_client
        .process_transaction(tx)
        .await
        .unwrap_err()
        .unwrap();

    assert_eq!(
        error,
        TransactionError::InstructionError(
            0,
            InstructionError::InvalidSeeds
        )
    );
}

#[tokio::test]
async fn test_withdraw_lamport_success() {
    let program_id = Pubkey::new_from_array([5; 32]);
    let pt = setup(&program_id);

    let (
        mut banks_client,
        payer,
        recent_blockhash
    ) = pt.start().await;

    let bank_account_owner = payer;
    setup_new_bank_account(
        &mut banks_client,
        &bank_account_owner,
        &program_id,
        recent_blockhash
    ).await;

    let bank_account_pda = Pubkey::try_find_program_address(
        &[
            b"user_bank_account",
            bank_account_owner.pubkey().as_ref()
        ],
        &program_id
    ).unwrap().0;

    let transfer_lamport_ix = transfer_lamports(
        &bank_account_owner.pubkey(),
        &bank_account_pda,
        sol_to_lamports(1.0)
    );
    let withdraw_lamport_ix = create_withdraw_lamports(
        &bank_account_owner.pubkey(),
        &bank_account_pda,
        &bank_account_owner.pubkey(),
        &program_id,
        &sol_to_lamports(0.5)
    );

    let tx = Transaction::new_signed_with_payer(
        &[
            transfer_lamport_ix,
            withdraw_lamport_ix
        ],
        Some(&bank_account_owner.pubkey()),
        &[&bank_account_owner],
        recent_blockhash
    );

    banks_client
        .process_transaction(tx)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_withdraw_lamport_fail_insufficient_balance() {
    let program_id = Pubkey::new_from_array([5; 32]);
    let pt = setup(&program_id);

    let (
        mut banks_client,
        payer,
        recent_blockhash
    ) = pt.start().await;

    let bank_account_owner = payer;
    setup_new_bank_account(
        &mut banks_client,
        &bank_account_owner,
        &program_id,
        recent_blockhash
    ).await;

    let bank_account_pda = Pubkey::try_find_program_address(
        &[
            b"user_bank_account",
            bank_account_owner.pubkey().as_ref()
        ],
        &program_id
    ).unwrap().0;

    let transfer_lamport_ix = transfer_lamports(
        &bank_account_owner.pubkey(),
        &bank_account_pda,
        sol_to_lamports(1.0)
    );
    let withdraw_lamport_ix = create_withdraw_lamports(
        &bank_account_owner.pubkey(),
        &bank_account_pda,
        &bank_account_owner.pubkey(),
        &program_id,
        &sol_to_lamports(1.000001)
    );

    let tx = Transaction::new_signed_with_payer(
        &[
            transfer_lamport_ix,
            withdraw_lamport_ix
        ],
        Some(&bank_account_owner.pubkey()),
        &[&bank_account_owner],
        recent_blockhash
    );

    let error = banks_client
        .process_transaction(tx)
        .await
        .unwrap_err()
        .unwrap();

    assert_eq!(
        error,
        TransactionError::InstructionError(
            1,
            InstructionError::Custom(
                BankError::InsufficientLamportBalance as u32
            )
        )
    );
}

#[tokio::test]
async fn test_withdraw_spl_token_success() {
    let program_id = Pubkey::new_from_array([2; 32]);
    let mut pt = setup(&program_id);

    let operator = Keypair::new();
    pt.add_account(
        operator.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(100.0),
            owner: Pubkey::default(),
            data: vec![],
            executable: false,
            rent_epoch: Epoch::default()
        }
    );

    let mint_account = Pubkey::new_from_array([3;32]);
    setup_new_mint_account(
        &mut pt,
        &TOKEN_STANDARD_PROGRAM,
        &mint_account,
        &operator.pubkey(),
        1000_00u64
    );

    let operator_token_account = Pubkey::new_from_array([4; 32]);
    setup_new_token_account(
        &mut pt,
        &TOKEN_STANDARD_PROGRAM,
        &mint_account,
        &operator_token_account,
        &operator.pubkey(),
        0_00u64
    );

    let bank_account_pda = Pubkey::try_find_program_address(
        &[
            b"user_bank_account",
            operator.pubkey().as_ref()
        ],
        &program_id
    ).unwrap().0;

    setup_new_associated_token_account(
        &mut pt,
        &TOKEN_STANDARD_PROGRAM,
        &mint_account,
        &bank_account_pda,
        1000_00u64
    );

    let (
        mut banks_client,
        payer,
        recent_blockhash
    ) = pt.start().await;

    setup_new_bank_account(
        &mut banks_client,
        &operator,
        &program_id,
        recent_blockhash
    ).await;

    let bank_account_associated_token_account = Pubkey::try_find_program_address(
        &[
            bank_account_pda.as_ref(),
            TOKEN_STANDARD_PROGRAM.as_ref(),
            mint_account.as_ref()
        ],
        &ASSOCIATED_TOKEN_PROGRAM
    ).unwrap().0;
 
    let ix = create_withdraw_spl_tokens(
        &operator.pubkey(),
        &bank_account_pda,
        &bank_account_associated_token_account,
        &mint_account,
        &operator_token_account,
        &program_id,
        &100_00u64
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[
            &payer,
            &operator
        ],
        recent_blockhash
    );

    banks_client
        .process_transaction(tx)
        .await
        .unwrap();

    let operator_token_account_data = banks_client
        .get_account(operator_token_account)
        .await.unwrap().unwrap().data;
    let bank_account_associated_token_account_data = banks_client
        .get_account(bank_account_associated_token_account)
        .await.unwrap().unwrap().data;

    let operator_token_account_info = TokenAccount::unpack(
        operator_token_account_data.as_slice()
    ).unwrap();
    let bank_account_associated_token_account_info = TokenAccount::unpack(
        bank_account_associated_token_account_data.as_slice()
    ).unwrap();

    assert_eq!(
        operator_token_account_info.amount,
        100_00u64,
        "Operator token balance mismatch."
    );
    assert_eq!(
        bank_account_associated_token_account_info.amount,
        900_00u64,
        "Bank-account token balance mismatch."
    );
}

#[tokio::test]
async fn test_withdraw_spl_token_fail_invalid_seeds() {
    let program_id = Pubkey::new_from_array([2; 32]);
    let mut pt = setup(&program_id);

    let operator = Keypair::new();
    pt.add_account(
        operator.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(100.0),
            owner: Pubkey::default(),
            data: vec![],
            executable: false,
            rent_epoch: Epoch::default()
        }
    );

    let mint_account = Pubkey::new_from_array([3;32]);
    setup_new_mint_account(
        &mut pt,
        &TOKEN_STANDARD_PROGRAM,
        &mint_account,
        &operator.pubkey(),
        1000_00u64
    );

    let operator_token_account = Pubkey::new_from_array([4; 32]);
    setup_new_token_account(
        &mut pt,
        &TOKEN_STANDARD_PROGRAM,
        &mint_account,
        &operator_token_account,
        &operator.pubkey(),
        0_00u64
    );

    let bank_account_pda = Pubkey::try_find_program_address(
        &[
            b"user_bank_account",
            operator.pubkey().as_ref()
        ],
        &program_id
    ).unwrap().0;

    setup_new_associated_token_account(
        &mut pt,
        &TOKEN_STANDARD_PROGRAM,
        &mint_account,
        &bank_account_pda,
        1000_00u64
    );

    let (
        mut banks_client,
        payer,
        recent_blockhash
    ) = pt.start().await;

    setup_new_bank_account(
        &mut banks_client,
        &operator,
        &program_id,
        recent_blockhash
    ).await;

    let bank_account_associated_token_account = Pubkey::try_find_program_address(
        &[
            Pubkey::default().as_ref(),
            TOKEN_STANDARD_PROGRAM.as_ref(),
            mint_account.as_ref()
        ],
        &ASSOCIATED_TOKEN_PROGRAM
    ).unwrap().0;
 
    let ix = create_withdraw_spl_tokens(
        &operator.pubkey(),
        &bank_account_pda,
        &bank_account_associated_token_account,
        &mint_account,
        &operator_token_account,
        &program_id,
        &100_00u64
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[
            &payer,
            &operator
        ],
        recent_blockhash
    );

    let error = banks_client
        .process_transaction(tx)
        .await
        .unwrap_err()
        .unwrap();

    assert_eq!(
        error,
        TransactionError::InstructionError(
            0,
            InstructionError::InvalidSeeds
        )
    );
}

#[tokio::test]
async fn test_withdraw_lamports_using_ed25519_success_1() {
    let program_id = Pubkey::new_from_array([2; 32]);
    let mut pt = setup(&program_id);

    let message_signer = Keypair::from_bytes(
        &[
            159,42,51,158,177,31,236,33,199,251,245,169,11,226,48,147,119,9,180,119,251,52,
            136,183,83,36,3,12,120,40,177,57,187,220,42,181,173,60,36,199,230,65,125,124,22,
            8,191,157,169,169,85,113,83,0,79,147,213,225,127,24,199,48,221,200
        ]
    ).unwrap(); // "DeKxTUZrgjpUzNjibLc8kvbByz9e37BEJ8Ce7xDairhV"
    pt.add_account(
        message_signer.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(100.0),
            owner: Pubkey::default(),
            rent_epoch: Epoch::default(),
            executable: false,
            data: vec![]
        }
    );

    let to = Keypair::from_bytes(
        &[
            237,227,10,102,176,81,227,2,143,72,178,176,123,49,168,231,31,
            164,112,111,25,25,196,116,155,99,155,16,225,248,60,255,54,140,
            26,77,149,64,206,192,130,179,65,73,200,27,46,201,49,21,157,36,
            117,177,107,131,121,11,228,101,173,11,51,156
        ]
    ).unwrap(); // "4fvvYqkcG5izqVNuMKyR4ChC7wAbwee7xG1DJW7DjDPZ"
    pt.add_account(
        to.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(1.0),
            owner: Pubkey::default(),
            rent_epoch: Epoch::default(),
            executable: false,
            data: vec![]
        }
    );

    let (
        mut banks_client,
        payer,
        recent_blockhash
    ) = pt.start().await;

    let bank_account_pda = Pubkey::try_find_program_address(
        &[
            b"user_bank_account",
            message_signer.pubkey().as_ref()
        ],
        &program_id
    ).unwrap().0;
    setup_new_bank_account(
        &mut banks_client,
        &message_signer,
        &program_id,
        recent_blockhash
    ).await;

    let send_lamport_to_bank_account_ix = transfer_lamports(
        &message_signer.pubkey(),
        &bank_account_pda,
        30 * LAMPORTS_PER_SOL
    );

    let tx = Transaction::new_signed_with_payer(
        &[send_lamport_to_bank_account_ix],
        Some(&payer.pubkey()),
        &[
            &payer,
            &message_signer
        ],
        recent_blockhash
    );

    banks_client
        .process_transaction(tx)
        .await
        .unwrap();


    let _message = "4fvvYqkcG5izqVNuMKyR4ChC7wAbwee7xG1DJW7DjDPZ,1500000000,PooriaGG 😃";

    let ed25519_signature_verification_instruction = Instruction {
        program_id: solana_sdk::ed25519_program::ID,
        accounts: vec![],
        data: vec![
            1,0,48,0,255,255,16,0,255,255,112,0,64,0,255,255,187,220,42,181,173,60,36,199,230,65,125,124,22,8,191,157,169,
            169,85,113,83,0,79,147,213,225,127,24,199,48,221,200,200,251,137,102,255,74,244,207,8,44,211,53,225,188,191,241,
            98,236,68,152,169,155,198,174,188,31,92,100,146,18,57,0,15,224,180,15,84,19,119,172,117,108,201,10,78,70,198,54,
            215,109,7,149,99,100,189,213,112,197,131,197,244,56,141,8,52,102,118,118,89,113,107,99,71,53,105,122,113,86,78,117,
            77,75,121,82,52,67,104,67,55,119,65,98,119,101,101,55,120,71,49,68,74,87,55,68,106,68,80,90,44,49,53,48,48,48,48,48,
            48,48,48,44,80,111,111,114,105,97,71,71,32,240,159,152,131
        ]
    };

    let withdraw_lamports_using_ed25519_ix = create_withdraw_lamports_using_ed25519_signature(
        &bank_account_pda,
        &to.pubkey(),
        &to.pubkey(),
        &to.pubkey(),
        &SYSTEM_PROGRAM_ID,
        Some(&MEMO_PROGRAM_ID),
        &program_id
    );

    let ed = ed25519_signature_verification_instruction.clone();

    let tx = Transaction::new_signed_with_payer(
        &[
            ed25519_signature_verification_instruction,
            withdraw_lamports_using_ed25519_ix
        ],
        Some(&payer.pubkey()),
        &[
            &payer,
            &to
        ],
        recent_blockhash
    );

    banks_client
        .process_transaction(tx)
        .await
        .unwrap();

    let bank_account_data = banks_client
        .get_account(bank_account_pda)
        .await
        .unwrap()
        .unwrap()
        .data;
    let bank_account_info = try_from_slice_unchecked::<UserBankAccount>(
        &bank_account_data
    ).unwrap();

    let signature_info = bank_account_info
        .signatures
        .get(0usize)
        .unwrap();

    let ed25519_data = ed.data;

    let ed25519_msg = ed25519_data.get(112..).unwrap().to_vec();
    assert_eq!(
        signature_info.message,
        ed25519_msg,
        "Message mismatch."
    );

    let ed25519_signature: [u8; 64] = ed25519_data.get(48..112).unwrap().try_into().unwrap();
    assert_eq!(
        signature_info.signature,
        ed25519_signature,
        "Signature mismatch."
    );

    assert!(signature_info.is_ok);

    let bank_account_balance = banks_client
        .get_balance(bank_account_pda)
        .await
        .unwrap();

    assert_eq!(
        bank_account_balance,
        28_502_289_840u64,
        "Bank-Account balance mismatch."
    );
    
    let to_account_balance = banks_client
        .get_balance(to.pubkey())
        .await
        .unwrap();

    assert_eq!(
        to_account_balance,
        2_498_983_840u64,
        "To-Account balance mismatch."
    );
}

#[tokio::test]
async fn test_withdraw_lamports_using_ed25519_success_2() {
    let program_id = Pubkey::new_from_array([2; 32]);
    let mut pt = setup(&program_id);

    let message_signer = Keypair::from_bytes(
        &[
            159,42,51,158,177,31,236,33,199,251,245,169,11,226,48,147,119,9,180,119,251,52,
            136,183,83,36,3,12,120,40,177,57,187,220,42,181,173,60,36,199,230,65,125,124,22,
            8,191,157,169,169,85,113,83,0,79,147,213,225,127,24,199,48,221,200
        ]
    ).unwrap(); // "DeKxTUZrgjpUzNjibLc8kvbByz9e37BEJ8Ce7xDairhV"
    pt.add_account(
        message_signer.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(100.0),
            owner: Pubkey::default(),
            rent_epoch: Epoch::default(),
            executable: false,
            data: vec![]
        }
    );

    let to = Keypair::from_bytes(
        &[
            237,227,10,102,176,81,227,2,143,72,178,176,123,49,168,231,31,
            164,112,111,25,25,196,116,155,99,155,16,225,248,60,255,54,140,
            26,77,149,64,206,192,130,179,65,73,200,27,46,201,49,21,157,36,
            117,177,107,131,121,11,228,101,173,11,51,156
        ]
    ).unwrap(); // "4fvvYqkcG5izqVNuMKyR4ChC7wAbwee7xG1DJW7DjDPZ"
    pt.add_account(
        to.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(1.0),
            owner: Pubkey::default(),
            rent_epoch: Epoch::default(),
            executable: false,
            data: vec![]
        }
    );

    let (
        mut banks_client,
        payer,
        recent_blockhash
    ) = pt.start().await;

    let bank_account_pda = Pubkey::try_find_program_address(
        &[
            b"user_bank_account",
            message_signer.pubkey().as_ref()
        ],
        &program_id
    ).unwrap().0;
    setup_new_bank_account(
        &mut banks_client,
        &message_signer,
        &program_id,
        recent_blockhash
    ).await;

    let send_lamport_to_bank_account_ix = transfer_lamports(
        &message_signer.pubkey(),
        &bank_account_pda,
        30 * LAMPORTS_PER_SOL
    );

    let tx = Transaction::new_signed_with_payer(
        &[send_lamport_to_bank_account_ix],
        Some(&payer.pubkey()),
        &[
            &payer,
            &message_signer
        ],
        recent_blockhash
    );

    banks_client
        .process_transaction(tx)
        .await
        .unwrap();


    let _message = "4fvvYqkcG5izqVNuMKyR4ChC7wAbwee7xG1DJW7DjDPZ,50000000000,PooriaGG 😃";

    let ed25519_signature_verification_instruction = Instruction {
        program_id: solana_sdk::ed25519_program::ID,
        accounts: vec![],
        data: vec![
            1,0,48,0,255,255,16,0,255,255,112,0,70,0,255,255,187,220,42,181,173,60,36,199,230,65,125,124,22,8,
            191,157,169,169,85,113,83,0,79,147,213,225,127,24,199,48,221,200,207,162,88,16,25,188,185,93,161,90,
            6,62,215,17,139,27,4,111,147,95,173,85,151,17,243,129,217,239,167,44,50,228,144,63,81,99,163,35,107,
            189,216,217,184,81,239,40,100,240,183,180,199,149,19,91,145,134,52,42,202,19,33,245,165,11,52,102,118,
            118,89,113,107,99,71,53,105,122,113,86,78,117,77,75,121,82,52,67,104,67,55,119,65,98,119,101,101,55,120,
            71,49,68,74,87,55,68,106,68,80,90,44,53,48,48,48,48,48,48,48,48,48,48,44,80,111,111,114,105,97,71,71,32,
            240,159,152,131
        ]
    };

    let withdraw_lamports_using_ed25519_ix = create_withdraw_lamports_using_ed25519_signature(
        &bank_account_pda,
        &to.pubkey(),
        &to.pubkey(),
        &to.pubkey(),
        &SYSTEM_PROGRAM_ID,
        Some(&MEMO_PROGRAM_ID),
        &program_id
    );

    let tx = Transaction::new_signed_with_payer(
        &[
            ed25519_signature_verification_instruction,
            withdraw_lamports_using_ed25519_ix
        ],
        Some(&payer.pubkey()),
        &[
            &payer,
            &to
        ],
        recent_blockhash
    );

    banks_client
        .process_transaction(tx)
        .await
        .unwrap();

    let bank_account_data = banks_client
        .get_account(bank_account_pda)
        .await
        .unwrap()
        .unwrap()
        .data;
    let bank_account_info = try_from_slice_unchecked::<UserBankAccount>(
        &bank_account_data
    ).unwrap();

    let signature_info = bank_account_info
        .signatures
        .get(0usize)
        .unwrap();

    assert_eq!(
        signature_info.is_ok,
        false,
        "Flag is invalid."
    );
}

#[tokio::test]
async fn test_withdraw_lamports_using_ed25519_success_3() {
    let program_id = Pubkey::new_from_array([2; 32]);
    let mut pt = setup(&program_id);

    let message_signer = Keypair::from_bytes(
        &[
            159,42,51,158,177,31,236,33,199,251,245,169,11,226,48,147,119,9,180,119,251,52,
            136,183,83,36,3,12,120,40,177,57,187,220,42,181,173,60,36,199,230,65,125,124,22,
            8,191,157,169,169,85,113,83,0,79,147,213,225,127,24,199,48,221,200
        ]
    ).unwrap(); // "DeKxTUZrgjpUzNjibLc8kvbByz9e37BEJ8Ce7xDairhV"
    pt.add_account(
        message_signer.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(100.0),
            owner: Pubkey::default(),
            rent_epoch: Epoch::default(),
            executable: false,
            data: vec![]
        }
    );

    let to = Keypair::from_bytes(
        &[
            237,227,10,102,176,81,227,2,143,72,178,176,123,49,168,231,31,
            164,112,111,25,25,196,116,155,99,155,16,225,248,60,255,54,140,
            26,77,149,64,206,192,130,179,65,73,200,27,46,201,49,21,157,36,
            117,177,107,131,121,11,228,101,173,11,51,156
        ]
    ).unwrap(); // "4fvvYqkcG5izqVNuMKyR4ChC7wAbwee7xG1DJW7DjDPZ"
    pt.add_account(
        to.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(1.0),
            owner: Pubkey::default(),
            rent_epoch: Epoch::default(),
            executable: false,
            data: vec![]
        }
    );

    let (
        mut banks_client,
        payer,
        recent_blockhash
    ) = pt.start().await;

    let bank_account_pda = Pubkey::try_find_program_address(
        &[
            b"user_bank_account",
            message_signer.pubkey().as_ref()
        ],
        &program_id
    ).unwrap().0;
    setup_new_bank_account(
        &mut banks_client,
        &message_signer,
        &program_id,
        recent_blockhash
    ).await;

    let send_lamport_to_bank_account_ix = transfer_lamports(
        &message_signer.pubkey(),
        &bank_account_pda,
        30 * LAMPORTS_PER_SOL
    );

    let tx = Transaction::new_signed_with_payer(
        &[send_lamport_to_bank_account_ix],
        Some(&payer.pubkey()),
        &[
            &payer,
            &message_signer
        ],
        recent_blockhash
    );

    banks_client
        .process_transaction(tx)
        .await
        .unwrap();


    let _message = "4fvvYqkcG5izqVNuMKyR4ChC7wAbwee7xG1DJW7DjDPZ,50000000000,PooriaGG 😃";

    let ed25519_signature_verification_instruction = Instruction {
        program_id: solana_sdk::ed25519_program::ID,
        accounts: vec![],
        data: vec![
            1,0,48,0,255,255,16,0,255,255,112,0,70,0,255,255,187,220,42,181,173,60,36,199,230,65,125,124,22,8,
            191,157,169,169,85,113,83,0,79,147,213,225,127,24,199,48,221,200,207,162,88,16,25,188,185,93,161,90,
            6,62,215,17,139,27,4,111,147,95,173,85,151,17,243,129,217,239,167,44,50,228,144,63,81,99,163,35,107,
            189,216,217,184,81,239,40,100,240,183,180,199,149,19,91,145,134,52,42,202,19,33,245,165,11,52,102,118,
            118,89,113,107,99,71,53,105,122,113,86,78,117,77,75,121,82,52,67,104,67,55,119,65,98,119,101,101,55,120,
            71,49,68,74,87,55,68,106,68,80,90,44,53,48,48,48,48,48,48,48,48,48,48,44,80,111,111,114,105,97,71,71,32,
            240,159,152,131
        ]
    };

    let withdraw_lamports_using_ed25519_ix = create_withdraw_lamports_using_ed25519_signature(
        &bank_account_pda,
        &to.pubkey(),
        &to.pubkey(),
        &to.pubkey(),
        &SYSTEM_PROGRAM_ID,
        Some(&MEMO_PROGRAM_ID),
        &program_id
    );

    let tx = Transaction::new_signed_with_payer(
        &[
            ed25519_signature_verification_instruction.clone(),
            withdraw_lamports_using_ed25519_ix.clone(),
            ed25519_signature_verification_instruction,
            withdraw_lamports_using_ed25519_ix
        ],
        Some(&payer.pubkey()),
        &[
            &payer,
            &to
        ],
        recent_blockhash
    );

    banks_client
        .process_transaction(tx)
        .await
        .unwrap();

    let bank_account_data = banks_client
        .get_account(bank_account_pda)
        .await
        .unwrap()
        .unwrap()
        .data;
    let bank_account_info = try_from_slice_unchecked::<UserBankAccount>(
        &bank_account_data
    ).unwrap();

    assert_eq!(
        bank_account_info.signatures.len(),
        2usize
    );

    let sig_info_1 = bank_account_info.signatures.get(0usize).unwrap();
    let sig_info_2 = bank_account_info.signatures.get(1usize).unwrap();

    assert_eq!(
        sig_info_1.is_ok,
        false
    );
    assert_eq!(
        sig_info_2.is_ok,
        false
    );

    assert_eq!(
        sig_info_1.signature,
        sig_info_2.signature
    );
}

#[tokio::test]
async fn test_withdraw_lamports_using_ed25519_fail_failed_to_get_edd25519_ix() {
    let program_id = Pubkey::new_from_array([2; 32]);
    let mut pt = setup(&program_id);

    let message_signer = Keypair::from_bytes(
        &[
            159,42,51,158,177,31,236,33,199,251,245,169,11,226,48,147,119,9,180,119,251,52,
            136,183,83,36,3,12,120,40,177,57,187,220,42,181,173,60,36,199,230,65,125,124,22,
            8,191,157,169,169,85,113,83,0,79,147,213,225,127,24,199,48,221,200
        ]
    ).unwrap(); // "DeKxTUZrgjpUzNjibLc8kvbByz9e37BEJ8Ce7xDairhV"
    pt.add_account(
        message_signer.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(100.0),
            owner: Pubkey::default(),
            rent_epoch: Epoch::default(),
            executable: false,
            data: vec![]
        }
    );

    let to = Keypair::from_bytes(
        &[
            237,227,10,102,176,81,227,2,143,72,178,176,123,49,168,231,31,
            164,112,111,25,25,196,116,155,99,155,16,225,248,60,255,54,140,
            26,77,149,64,206,192,130,179,65,73,200,27,46,201,49,21,157,36,
            117,177,107,131,121,11,228,101,173,11,51,156
        ]
    ).unwrap(); // "4fvvYqkcG5izqVNuMKyR4ChC7wAbwee7xG1DJW7DjDPZ"
    pt.add_account(
        to.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(1.0),
            owner: Pubkey::default(),
            rent_epoch: Epoch::default(),
            executable: false,
            data: vec![]
        }
    );

    let (
        mut banks_client,
        payer,
        recent_blockhash
    ) = pt.start().await;

    let bank_account_pda = Pubkey::try_find_program_address(
        &[
            b"user_bank_account",
            message_signer.pubkey().as_ref()
        ],
        &program_id
    ).unwrap().0;
    setup_new_bank_account(
        &mut banks_client,
        &message_signer,
        &program_id,
        recent_blockhash
    ).await;

    let send_lamport_to_bank_account_ix = transfer_lamports(
        &message_signer.pubkey(),
        &bank_account_pda,
        30 * LAMPORTS_PER_SOL
    );

    let tx = Transaction::new_signed_with_payer(
        &[send_lamport_to_bank_account_ix],
        Some(&payer.pubkey()),
        &[
            &payer,
            &message_signer
        ],
        recent_blockhash
    );

    banks_client
        .process_transaction(tx)
        .await
        .unwrap();

    let withdraw_lamports_using_ed25519_ix = create_withdraw_lamports_using_ed25519_signature(
        &bank_account_pda,
        &to.pubkey(),
        &to.pubkey(),
        &to.pubkey(),
        &SYSTEM_PROGRAM_ID,
        Some(&MEMO_PROGRAM_ID),
        &program_id
    );

    let tx = Transaction::new_signed_with_payer(
        &[withdraw_lamports_using_ed25519_ix],
        Some(&payer.pubkey()),
        &[
            &payer,
            &to
        ],
        recent_blockhash
    );

    let error = banks_client
        .process_transaction(tx)
        .await
        .unwrap_err()
        .unwrap();

    assert_eq!(
        error,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(
                BankError::FailedToGetEd25519Instruction as u32
            )
        )
    );
}

#[tokio::test]
async fn test_withdraw_lamports_using_ed25519_fail_invalid_ed25519_ix() {
    let program_id = Pubkey::new_from_array([2; 32]);
    let mut pt = setup(&program_id);

    let message_signer = Keypair::from_bytes(
        &[
            159,42,51,158,177,31,236,33,199,251,245,169,11,226,48,147,119,9,180,119,251,52,
            136,183,83,36,3,12,120,40,177,57,187,220,42,181,173,60,36,199,230,65,125,124,22,
            8,191,157,169,169,85,113,83,0,79,147,213,225,127,24,199,48,221,200
        ]
    ).unwrap(); // "DeKxTUZrgjpUzNjibLc8kvbByz9e37BEJ8Ce7xDairhV"
    pt.add_account(
        message_signer.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(100.0),
            owner: Pubkey::default(),
            rent_epoch: Epoch::default(),
            executable: false,
            data: vec![]
        }
    );

    let to = Keypair::from_bytes(
        &[
            237,227,10,102,176,81,227,2,143,72,178,176,123,49,168,231,31,
            164,112,111,25,25,196,116,155,99,155,16,225,248,60,255,54,140,
            26,77,149,64,206,192,130,179,65,73,200,27,46,201,49,21,157,36,
            117,177,107,131,121,11,228,101,173,11,51,156
        ]
    ).unwrap(); // "4fvvYqkcG5izqVNuMKyR4ChC7wAbwee7xG1DJW7DjDPZ"
    pt.add_account(
        to.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(1.0),
            owner: Pubkey::default(),
            rent_epoch: Epoch::default(),
            executable: false,
            data: vec![]
        }
    );

    let (
        mut banks_client,
        payer,
        recent_blockhash
    ) = pt.start().await;

    let bank_account_pda = Pubkey::try_find_program_address(
        &[
            b"user_bank_account",
            message_signer.pubkey().as_ref()
        ],
        &program_id
    ).unwrap().0;
    setup_new_bank_account(
        &mut banks_client,
        &message_signer,
        &program_id,
        recent_blockhash
    ).await;

    let send_lamport_to_bank_account_ix = transfer_lamports(
        &message_signer.pubkey(),
        &bank_account_pda,
        30 * LAMPORTS_PER_SOL
    );

    let tx = Transaction::new_signed_with_payer(
        &[send_lamport_to_bank_account_ix],
        Some(&payer.pubkey()),
        &[
            &payer,
            &message_signer
        ],
        recent_blockhash
    );

    banks_client
        .process_transaction(tx)
        .await
        .unwrap();


    let withdraw_lamports_using_ed25519_ix = create_withdraw_lamports_using_ed25519_signature(
        &bank_account_pda,
        &to.pubkey(),
        &to.pubkey(),
        &to.pubkey(),
        &SYSTEM_PROGRAM_ID,
        None,
        &program_id
    );

    let tx = Transaction::new_signed_with_payer(
        &[
            transfer_lamports(
                &payer.pubkey(),
                &to.pubkey(),
                1_000_000
            ),
            withdraw_lamports_using_ed25519_ix
        ],
        Some(&payer.pubkey()),
        &[
            &payer,
            &to
        ],
        recent_blockhash
    );

    let error = banks_client
        .process_transaction(tx)
        .await
        .unwrap_err()
        .unwrap();

    assert_eq!(
        error,
        TransactionError::InstructionError(
            1,
            InstructionError::Custom(
                BankError::InvalidEd25519SignatureVerificationInstruction as u32
            )
        )
    );
}

#[tokio::test]
async fn test_withdraw_lamports_using_ed25519_fail_invalid_message_v1() {
    let program_id = Pubkey::new_from_array([2; 32]);
    let mut pt = setup(&program_id);

    let message_signer = Keypair::from_bytes(
        &[
            159,42,51,158,177,31,236,33,199,251,245,169,11,226,48,147,119,9,180,119,251,52,
            136,183,83,36,3,12,120,40,177,57,187,220,42,181,173,60,36,199,230,65,125,124,22,
            8,191,157,169,169,85,113,83,0,79,147,213,225,127,24,199,48,221,200
        ]
    ).unwrap(); // "DeKxTUZrgjpUzNjibLc8kvbByz9e37BEJ8Ce7xDairhV"
    pt.add_account(
        message_signer.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(100.0),
            owner: Pubkey::default(),
            rent_epoch: Epoch::default(),
            executable: false,
            data: vec![]
        }
    );

    let to = Keypair::from_bytes(
        &[
            237,227,10,102,176,81,227,2,143,72,178,176,123,49,168,231,31,
            164,112,111,25,25,196,116,155,99,155,16,225,248,60,255,54,140,
            26,77,149,64,206,192,130,179,65,73,200,27,46,201,49,21,157,36,
            117,177,107,131,121,11,228,101,173,11,51,156
        ]
    ).unwrap(); // "4fvvYqkcG5izqVNuMKyR4ChC7wAbwee7xG1DJW7DjDPZ"
    pt.add_account(
        to.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(1.0),
            owner: Pubkey::default(),
            rent_epoch: Epoch::default(),
            executable: false,
            data: vec![]
        }
    );

    let (
        mut banks_client,
        payer,
        recent_blockhash
    ) = pt.start().await;

    let bank_account_pda = Pubkey::try_find_program_address(
        &[
            b"user_bank_account",
            message_signer.pubkey().as_ref()
        ],
        &program_id
    ).unwrap().0;
    setup_new_bank_account(
        &mut banks_client,
        &message_signer,
        &program_id,
        recent_blockhash
    ).await;

    let send_lamport_to_bank_account_ix = transfer_lamports(
        &message_signer.pubkey(),
        &bank_account_pda,
        30 * LAMPORTS_PER_SOL
    );

    let tx = Transaction::new_signed_with_payer(
        &[send_lamport_to_bank_account_ix],
        Some(&payer.pubkey()),
        &[
            &payer,
            &message_signer
        ],
        recent_blockhash
    );

    banks_client
        .process_transaction(tx)
        .await
        .unwrap();


    let _message = "4fvvYqkcG5izqVNuMKyR4ChC7wAbwee7xG1DJW7DjDPZ,1500000000";

    let ed25519_signature_verification_instruction = Instruction {
        program_id: solana_sdk::ed25519_program::ID,
        accounts: vec![],
        data: vec![
            1,0,48,0,255,255,16,0,255,255,112,0,55,0,255,255,187,220,42,181,173,60,36,199,230,65,125,124,22,8,191,157,
            169,169,85,113,83,0,79,147,213,225,127,24,199,48,221,200,157,75,27,216,166,121,75,79,42,115,178,79,191,184,
            133,218,64,70,155,201,156,240,157,224,11,209,106,220,185,37,84,130,33,161,138,101,216,25,23,40,166,16,52,82,
            165,118,68,255,215,252,57,130,222,79,29,229,26,136,133,250,38,226,202,2,52,102,118,118,89,113,107,99,71,53,
            105,122,113,86,78,117,77,75,121,82,52,67,104,67,55,119,65,98,119,101,101,55,120,71,49,68,74,87,55,68,106,68,
            80,90,44,49,53,48,48,48,48,48,48,48,48
        ]
    };

    let withdraw_lamports_using_ed25519_ix = create_withdraw_lamports_using_ed25519_signature(
        &bank_account_pda,
        &to.pubkey(),
        &to.pubkey(),
        &to.pubkey(),
        &SYSTEM_PROGRAM_ID,
        Some(&MEMO_PROGRAM_ID),
        &program_id
    );

    let tx = Transaction::new_signed_with_payer(
        &[
            ed25519_signature_verification_instruction,
            withdraw_lamports_using_ed25519_ix
        ],
        Some(&payer.pubkey()),
        &[
            &payer,
            &to
        ],
        recent_blockhash
    );

    let error = banks_client
        .process_transaction(tx)
        .await
        .unwrap_err()
        .unwrap();

    assert_eq!(
        error,
        TransactionError::InstructionError(
            1,
            InstructionError::Custom(
                BankError::MessageV1ValidationFailed as u32
            )
        )
    );
}

#[tokio::test]
async fn test_withdraw_lamports_using_ed25519_fail_invalid_to_account() {
    let program_id = Pubkey::new_from_array([2; 32]);
    let mut pt = setup(&program_id);

    let message_signer = Keypair::from_bytes(
        &[
            159,42,51,158,177,31,236,33,199,251,245,169,11,226,48,147,119,9,180,119,251,52,
            136,183,83,36,3,12,120,40,177,57,187,220,42,181,173,60,36,199,230,65,125,124,22,
            8,191,157,169,169,85,113,83,0,79,147,213,225,127,24,199,48,221,200
        ]
    ).unwrap(); // "DeKxTUZrgjpUzNjibLc8kvbByz9e37BEJ8Ce7xDairhV"
    pt.add_account(
        message_signer.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(100.0),
            owner: Pubkey::default(),
            rent_epoch: Epoch::default(),
            executable: false,
            data: vec![]
        }
    );

    let to = Keypair::from_bytes(
        &[
            237,227,10,102,176,81,227,2,143,72,178,176,123,49,168,231,31,
            164,112,111,25,25,196,116,155,99,155,16,225,248,60,255,54,140,
            26,77,149,64,206,192,130,179,65,73,200,27,46,201,49,21,157,36,
            117,177,107,131,121,11,228,101,173,11,51,156
        ]
    ).unwrap(); // "4fvvYqkcG5izqVNuMKyR4ChC7wAbwee7xG1DJW7DjDPZ"
    pt.add_account(
        to.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(1.0),
            owner: Pubkey::default(),
            rent_epoch: Epoch::default(),
            executable: false,
            data: vec![]
        }
    );

    let fake_to = Keypair::new();
    pt.add_account(
        fake_to.pubkey(),
        SolanaAccount {
            lamports: Rent::default().minimum_balance(0),
            rent_epoch: Epoch::default(),
            executable: false,
            owner: Pubkey::default(),
            data: vec![]
        }
    );

    let (
        mut banks_client,
        payer,
        recent_blockhash
    ) = pt.start().await;

    let bank_account_pda = Pubkey::try_find_program_address(
        &[
            b"user_bank_account",
            message_signer.pubkey().as_ref()
        ],
        &program_id
    ).unwrap().0;
    setup_new_bank_account(
        &mut banks_client,
        &message_signer,
        &program_id,
        recent_blockhash
    ).await;

    let send_lamport_to_bank_account_ix = transfer_lamports(
        &message_signer.pubkey(),
        &bank_account_pda,
        30 * LAMPORTS_PER_SOL
    );

    let tx = Transaction::new_signed_with_payer(
        &[send_lamport_to_bank_account_ix],
        Some(&payer.pubkey()),
        &[
            &payer,
            &message_signer
        ],
        recent_blockhash
    );

    banks_client
        .process_transaction(tx)
        .await
        .unwrap();


    let _message = "4fvvYqkcG5izqVNuMKyR4ChC7wAbwee7xG1DJW7DjDPZ,50000000000,PooriaGG 😃";

    let ed25519_signature_verification_instruction = Instruction {
        program_id: solana_sdk::ed25519_program::ID,
        accounts: vec![],
        data: vec![
            1,0,48,0,255,255,16,0,255,255,112,0,70,0,255,255,187,220,42,181,173,60,36,199,230,65,125,124,22,8,
            191,157,169,169,85,113,83,0,79,147,213,225,127,24,199,48,221,200,207,162,88,16,25,188,185,93,161,90,
            6,62,215,17,139,27,4,111,147,95,173,85,151,17,243,129,217,239,167,44,50,228,144,63,81,99,163,35,107,
            189,216,217,184,81,239,40,100,240,183,180,199,149,19,91,145,134,52,42,202,19,33,245,165,11,52,102,118,
            118,89,113,107,99,71,53,105,122,113,86,78,117,77,75,121,82,52,67,104,67,55,119,65,98,119,101,101,55,120,
            71,49,68,74,87,55,68,106,68,80,90,44,53,48,48,48,48,48,48,48,48,48,48,44,80,111,111,114,105,97,71,71,32,
            240,159,152,131
        ]
    };

    let withdraw_lamports_using_ed25519_ix = create_withdraw_lamports_using_ed25519_signature(
        &bank_account_pda,
        &to.pubkey(),
        &fake_to.pubkey(),
        &to.pubkey(),
        &SYSTEM_PROGRAM_ID,
        Some(&MEMO_PROGRAM_ID),
        &program_id
    );

    let tx = Transaction::new_signed_with_payer(
        &[
            ed25519_signature_verification_instruction,
            withdraw_lamports_using_ed25519_ix
        ],
        Some(&payer.pubkey()),
        &[
            &payer,
            &to,
            &fake_to
        ],
        recent_blockhash
    );

    let error = banks_client
        .process_transaction(tx)
        .await
        .unwrap_err()
        .unwrap();

    assert_eq!(
        error,
        TransactionError::InstructionError(
            1,
            InstructionError::Custom(
                BankError::InvalidToPubkey as u32
            )
        )
    );
}

#[tokio::test]
async fn test_withdraw_spl_tokens_using_ed25519_success_1() {
    let program_id = Pubkey::new_from_array([2; 32]);
    let mut pt = setup(&program_id);

    let message_signer = Keypair::from_bytes(
        &[
            159,42,51,158,177,31,236,33,199,251,245,169,11,226,48,147,119,9,180,119,251,52,
            136,183,83,36,3,12,120,40,177,57,187,220,42,181,173,60,36,199,230,65,125,124,22,
            8,191,157,169,169,85,113,83,0,79,147,213,225,127,24,199,48,221,200
        ]
    ).unwrap(); // "DeKxTUZrgjpUzNjibLc8kvbByz9e37BEJ8Ce7xDairhV"

    let to = Keypair::from_bytes(
        &[
            237,227,10,102,176,81,227,2,143,72,178,176,123,49,168,231,31,
            164,112,111,25,25,196,116,155,99,155,16,225,248,60,255,54,140,
            26,77,149,64,206,192,130,179,65,73,200,27,46,201,49,21,157,36,
            117,177,107,131,121,11,228,101,173,11,51,156
        ]
    ).unwrap(); // "4fvvYqkcG5izqVNuMKyR4ChC7wAbwee7xG1DJW7DjDPZ"

    let (
        bank_account,
        bump
    ) = Pubkey::try_find_program_address(
        &[
            b"user_bank_account",
            message_signer.pubkey().to_bytes().as_slice()
        ],
        &program_id
    ).unwrap();

    pt.add_account(
        message_signer.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(1.0),
            rent_epoch: Epoch::default(),
            owner: Pubkey::default(),
            data: vec![],
            executable: false
        }
    );
    pt.add_account(
        to.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(2.0),
            rent_epoch: Epoch::default(),
            owner: Pubkey::default(),
            data: vec![],
            executable: false
        }
    );
    pt.add_account(
        bank_account,
        SolanaAccount {
            owner: program_id,
            lamports: sol_to_lamports(1.0),
            rent_epoch: Epoch::default(),
            data: UserBankAccount {
                discriminator: UserBankAccount::get_bank_account_discriminator(),
                bump,
                account_created_at: Clock::default().unix_timestamp,
                authority: message_signer.pubkey(),
                signatures: vec![]
            }.try_to_vec().unwrap(),
            executable: false
        }
    );

    let mint_account = Pubkey::new_from_array([3; 32]);
    let signer_token_account = Pubkey::new_from_array([4; 32]);
    let to_token_account = Pubkey::new_from_array([5; 32]);
    let bank_account_token_account = Pubkey::try_find_program_address(
        &[
            bank_account.to_bytes().as_slice(),
            TOKEN_STANDARD_PROGRAM.to_bytes().as_slice(),
            mint_account.to_bytes().as_slice()
        ],
        &ASSOCIATED_TOKEN_PROGRAM
    ).unwrap().0;
    
    setup_new_mint_account(
        &mut pt,
        &TOKEN_STANDARD_PROGRAM,
        &mint_account,
        &Keypair::new().pubkey(),
        1000_00u64
    );
    setup_new_token_account(
        &mut pt,
        &TOKEN_STANDARD_PROGRAM,
        &mint_account,
        &signer_token_account,
        &message_signer.pubkey(),
        500_00u64
    );
    setup_new_token_account(
        &mut pt,
        &TOKEN_STANDARD_PROGRAM,
        &mint_account,
        &to_token_account,
        &to.pubkey(),
        300_00u64
    );
    setup_new_token_account(
        &mut pt,
        &TOKEN_STANDARD_PROGRAM,
        &mint_account,
        &bank_account_token_account,
        &bank_account,
        200_00u64
    );

    let (
        mut banks_client,
        payer,
        recent_blockhash
    ) = pt.start().await;

    let _message: String = String::from(
        "4fvvYqkcG5izqVNuMKyR4ChC7wAbwee7xG1DJW7DjDPZ,CktRuQ2mttgRGkXJtyksdKHjUdc2C4TgDzyB98oEzy8,5000,PooriaGG 🤩"
    );

    let ed25519_signature_verification_instruction = Instruction {
        program_id: solana_sdk::ed25519_program::ID,
        accounts: vec![],
        data: vec![
            1,0,48,0,255,255,16,0,255,255,112,0,107,0,255,255,187,220,42,181,173,60,36,199,230,65,125,124,22,8,191,
            157,169,169,85,113,83,0,79,147,213,225,127,24,199,48,221,200,67,142,135,154,113,246,121,210,237,35,38,2,
            28,232,247,238,246,74,106,20,25,45,244,168,65,181,104,172,53,18,23,88,28,42,212,92,183,167,49,168,108,236,
            141,101,220,4,104,57,183,12,100,159,30,80,62,45,64,129,3,150,168,12,135,5,52,102,118,118,89,113,107,99,71,
            53,105,122,113,86,78,117,77,75,121,82,52,67,104,67,55,119,65,98,119,101,101,55,120,71,49,68,74,87,55,68,106,
            68,80,90,44,67,107,116,82,117,81,50,109,116,116,103,82,71,107,88,74,116,121,107,115,100,75,72,106,85,100,99,
            50,67,52,84,103,68,122,121,66,57,56,111,69,122,121,56,44,53,48,48,48,44,80,111,111,114,105,97,71,71,32,240,159,164,169
        ]
    };

    let withdraw_spl_tokens_using_ed25519 = create_withdraw_spl_tokens_using_ed25519_signature(
        &mint_account,
        &bank_account,
        &bank_account_token_account,
        &to.pubkey(),
        &to.pubkey(),
        &to_token_account,
        &TOKEN_STANDARD_PROGRAM,
        &SYSTEM_PROGRAM_ID,
        Some(&MEMO_PROGRAM_ID),
        &program_id
    );

    let tx = Transaction::new_signed_with_payer(
        &[
            ed25519_signature_verification_instruction.clone(),
            withdraw_spl_tokens_using_ed25519
        ],
        Some(&payer.pubkey()),
        &[
            &payer,
            &to
        ],
        recent_blockhash
    );

    banks_client
        .process_transaction(tx)
        .await
        .unwrap();

    let bank_account_data = banks_client
        .get_account(bank_account)
        .await
        .unwrap()
        .unwrap()
        .data;
    let bank_account_info = UserBankAccount::try_from_slice(
        &bank_account_data
    ).unwrap();

    let expected_signature: [u8; 64] = ed25519_signature_verification_instruction
        .data
        .get(48..112)
        .unwrap()
        .try_into()
        .unwrap();

    assert_eq!(
        expected_signature,
        bank_account_info
            .signatures
            .get(0)
            .unwrap()
            .signature,
        "Signature mismatch!"
    );

    assert!(bank_account_info.signatures[0].is_ok);

    let to_token_account_data = banks_client
        .get_account(to_token_account)
        .await
        .unwrap()
        .unwrap()
        .data;
    let to_token_account_info = TokenAccount::unpack(&to_token_account_data).unwrap();

    let bank_account_tokne_account_data = banks_client
        .get_account(bank_account_token_account)
        .await
        .unwrap()
        .unwrap()
        .data;
    let bank_account_token_account_info = TokenAccount::unpack(&bank_account_tokne_account_data).unwrap();

    assert_eq!(
        to_token_account_info.amount,
        350_00u64
    );

    assert_eq!(
        bank_account_token_account_info.amount,
        150_00u64
    );
}

#[tokio::test]
async fn test_withdraw_spl_tokens_using_ed25519_success_2() {
    let program_id = Pubkey::new_from_array([2; 32]);
    let mut pt = setup(&program_id);

    let message_signer = Keypair::from_bytes(
        &[
            159,42,51,158,177,31,236,33,199,251,245,169,11,226,48,147,119,9,180,119,251,52,
            136,183,83,36,3,12,120,40,177,57,187,220,42,181,173,60,36,199,230,65,125,124,22,
            8,191,157,169,169,85,113,83,0,79,147,213,225,127,24,199,48,221,200
        ]
    ).unwrap(); // "DeKxTUZrgjpUzNjibLc8kvbByz9e37BEJ8Ce7xDairhV"

    let to = Keypair::from_bytes(
        &[
            237,227,10,102,176,81,227,2,143,72,178,176,123,49,168,231,31,
            164,112,111,25,25,196,116,155,99,155,16,225,248,60,255,54,140,
            26,77,149,64,206,192,130,179,65,73,200,27,46,201,49,21,157,36,
            117,177,107,131,121,11,228,101,173,11,51,156
        ]
    ).unwrap(); // "4fvvYqkcG5izqVNuMKyR4ChC7wAbwee7xG1DJW7DjDPZ"

    let (
        bank_account,
        bump
    ) = Pubkey::try_find_program_address(
        &[
            b"user_bank_account",
            message_signer.pubkey().to_bytes().as_slice()
        ],
        &program_id
    ).unwrap();

    pt.add_account(
        message_signer.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(1.0),
            rent_epoch: Epoch::default(),
            owner: Pubkey::default(),
            data: vec![],
            executable: false
        }
    );
    pt.add_account(
        to.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(2.0),
            rent_epoch: Epoch::default(),
            owner: Pubkey::default(),
            data: vec![],
            executable: false
        }
    );
    pt.add_account(
        bank_account,
        SolanaAccount {
            owner: program_id,
            lamports: sol_to_lamports(1.0),
            rent_epoch: Epoch::default(),
            data: UserBankAccount {
                discriminator: UserBankAccount::get_bank_account_discriminator(),
                bump,
                account_created_at: Clock::default().unix_timestamp,
                authority: message_signer.pubkey(),
                signatures: vec![]
            }.try_to_vec().unwrap(),
            executable: false
        }
    );

    let mint_account = Pubkey::new_from_array([3; 32]);
    let signer_token_account = Pubkey::new_from_array([4; 32]);
    let to_token_account = Pubkey::new_from_array([5; 32]);
    let bank_account_token_account = Pubkey::try_find_program_address(
        &[
            bank_account.to_bytes().as_slice(),
            TOKEN_STANDARD_PROGRAM.to_bytes().as_slice(),
            mint_account.to_bytes().as_slice()
        ],
        &ASSOCIATED_TOKEN_PROGRAM
    ).unwrap().0;
    
    setup_new_mint_account(
        &mut pt,
        &TOKEN_STANDARD_PROGRAM,
        &mint_account,
        &Keypair::new().pubkey(),
        1000_00u64
    );
    setup_new_token_account(
        &mut pt,
        &TOKEN_STANDARD_PROGRAM,
        &mint_account,
        &signer_token_account,
        &message_signer.pubkey(),
        500_00u64
    );
    setup_new_token_account(
        &mut pt,
        &TOKEN_STANDARD_PROGRAM,
        &mint_account,
        &to_token_account,
        &to.pubkey(),
        300_00u64
    );
    setup_new_token_account(
        &mut pt,
        &TOKEN_STANDARD_PROGRAM,
        &mint_account,
        &bank_account_token_account,
        &bank_account,
        200_00u64
    );

    let (
        mut banks_client,
        payer,
        recent_blockhash
    ) = pt.start().await;

    let _message: String = String::from(
        "4fvvYqkcG5izqVNuMKyR4ChC7wAbwee7xG1DJW7DjDPZ,CktRuQ2mttgRGkXJtyksdKHjUdc2C4TgDzyB98oEzy8,50000,PooriaGG 🤩"
    );

    let ed25519_signature_verification_instruction = Instruction {
        program_id: solana_sdk::ed25519_program::ID,
        accounts: vec![],
        data: vec![
            1,0,48,0,255,255,16,0,255,255,112,0,108,0,255,255,187,220,42,181,173,60,36,199,230,65,125,124,22,8,191,157,169,169,
            85,113,83,0,79,147,213,225,127,24,199,48,221,200,55,78,28,211,84,230,134,173,195,114,9,205,40,231,223,7,61,8,212,64,
            156,21,81,42,211,136,180,255,112,43,54,224,245,21,81,14,97,192,71,63,231,247,247,138,35,155,250,190,29,223,228,80,
            108,120,155,234,213,192,204,191,136,173,124,3,52,102,118,118,89,113,107,99,71,53,105,122,113,86,78,117,77,75,121,82,
            52,67,104,67,55,119,65,98,119,101,101,55,120,71,49,68,74,87,55,68,106,68,80,90,44,67,107,116,82,117,81,50,109,116,116,
            103,82,71,107,88,74,116,121,107,115,100,75,72,106,85,100,99,50,67,52,84,103,68,122,121,66,57,56,111,69,122,121,56,44,
            53,48,48,48,48,44,80,111,111,114,105,97,71,71,32,240,159,164,169
        ]
    };

    let withdraw_spl_tokens_using_ed25519 = create_withdraw_spl_tokens_using_ed25519_signature(
        &mint_account,
        &bank_account,
        &bank_account_token_account,
        &to.pubkey(),
        &to.pubkey(),
        &to_token_account,
        &TOKEN_STANDARD_PROGRAM,
        &SYSTEM_PROGRAM_ID,
        Some(&MEMO_PROGRAM_ID),
        &program_id
    );

    let tx = Transaction::new_signed_with_payer(
        &[
            ed25519_signature_verification_instruction.clone(),
            withdraw_spl_tokens_using_ed25519
        ],
        Some(&payer.pubkey()),
        &[
            &payer,
            &to
        ],
        recent_blockhash
    );

    banks_client
        .process_transaction(tx)
        .await
        .unwrap();

    let bank_account_data = banks_client
        .get_account(bank_account)
        .await
        .unwrap()
        .unwrap()
        .data;
    let bank_account_info = UserBankAccount::try_from_slice(
        &bank_account_data
    ).unwrap();

    let expected_signature: [u8; 64] = ed25519_signature_verification_instruction
        .data
        .get(48..112)
        .unwrap()
        .try_into()
        .unwrap();

    assert!(!bank_account_info.signatures[0].is_ok);

    let to_token_account_data = banks_client
        .get_account(to_token_account)
        .await
        .unwrap()
        .unwrap()
        .data;
    let to_token_account_info = TokenAccount::unpack(&to_token_account_data).unwrap();

    let bank_account_tokne_account_data = banks_client
        .get_account(bank_account_token_account)
        .await
        .unwrap()
        .unwrap()
        .data;
    let bank_account_token_account_info = TokenAccount::unpack(&bank_account_tokne_account_data).unwrap();

    assert_eq!(
        to_token_account_info.amount,
        300_00u64
    );

    assert_eq!(
        bank_account_token_account_info.amount,
        200_00u64
    );
}

#[tokio::test]
async fn test_withdraw_spl_tokens_using_ed25519_success_3() {
    let program_id = Pubkey::new_from_array([2; 32]);
    let mut pt = setup(&program_id);

    let message_signer = Keypair::from_bytes(
        &[
            159,42,51,158,177,31,236,33,199,251,245,169,11,226,48,147,119,9,180,119,251,52,
            136,183,83,36,3,12,120,40,177,57,187,220,42,181,173,60,36,199,230,65,125,124,22,
            8,191,157,169,169,85,113,83,0,79,147,213,225,127,24,199,48,221,200
        ]
    ).unwrap(); // "DeKxTUZrgjpUzNjibLc8kvbByz9e37BEJ8Ce7xDairhV"

    let to = Keypair::from_bytes(
        &[
            237,227,10,102,176,81,227,2,143,72,178,176,123,49,168,231,31,
            164,112,111,25,25,196,116,155,99,155,16,225,248,60,255,54,140,
            26,77,149,64,206,192,130,179,65,73,200,27,46,201,49,21,157,36,
            117,177,107,131,121,11,228,101,173,11,51,156
        ]
    ).unwrap(); // "4fvvYqkcG5izqVNuMKyR4ChC7wAbwee7xG1DJW7DjDPZ"

    let (
        bank_account,
        bump
    ) = Pubkey::try_find_program_address(
        &[
            b"user_bank_account",
            message_signer.pubkey().to_bytes().as_slice()
        ],
        &program_id
    ).unwrap();

    pt.add_account(
        message_signer.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(1.0),
            rent_epoch: Epoch::default(),
            owner: Pubkey::default(),
            data: vec![],
            executable: false
        }
    );
    pt.add_account(
        to.pubkey(),
        SolanaAccount {
            lamports: sol_to_lamports(2.0),
            rent_epoch: Epoch::default(),
            owner: Pubkey::default(),
            data: vec![],
            executable: false
        }
    );
    pt.add_account(
        bank_account,
        SolanaAccount {
            owner: program_id,
            lamports: sol_to_lamports(1.0),
            rent_epoch: Epoch::default(),
            data: UserBankAccount {
                discriminator: UserBankAccount::get_bank_account_discriminator(),
                bump,
                account_created_at: Clock::default().unix_timestamp,
                authority: message_signer.pubkey(),
                signatures: vec![]
            }.try_to_vec().unwrap(),
            executable: false
        }
    );

    let mint_account = Pubkey::new_from_array([3; 32]);
    let signer_token_account = Pubkey::new_from_array([4; 32]);
    let to_token_account = Pubkey::new_from_array([5; 32]);
    let bank_account_token_account = Pubkey::try_find_program_address(
        &[
            bank_account.to_bytes().as_slice(),
            TOKEN_STANDARD_PROGRAM.to_bytes().as_slice(),
            mint_account.to_bytes().as_slice()
        ],
        &ASSOCIATED_TOKEN_PROGRAM
    ).unwrap().0;
    
    setup_new_mint_account(
        &mut pt,
        &TOKEN_STANDARD_PROGRAM,
        &mint_account,
        &Keypair::new().pubkey(),
        1000_00u64
    );
    setup_new_token_account(
        &mut pt,
        &TOKEN_STANDARD_PROGRAM,
        &mint_account,
        &signer_token_account,
        &message_signer.pubkey(),
        500_00u64
    );
    setup_new_token_account(
        &mut pt,
        &TOKEN_STANDARD_PROGRAM,
        &mint_account,
        &to_token_account,
        &to.pubkey(),
        300_00u64
    );
    setup_new_token_account(
        &mut pt,
        &TOKEN_STANDARD_PROGRAM,
        &mint_account,
        &bank_account_token_account,
        &bank_account,
        200_00u64
    );

    let (
        mut banks_client,
        payer,
        recent_blockhash
    ) = pt.start().await;

    let _message: String = String::from(
        "4fvvYqkcG5izqVNuMKyR4ChC7wAbwee7xG1DJW7DjDPZ,CktRuQ2mttgRGkXJtyksdKHjUdc2C4TgDzyB98oEzy8,50000,PooriaGG 🤩"
    );

    let ed25519_signature_verification_instruction = Instruction {
        program_id: solana_sdk::ed25519_program::ID,
        accounts: vec![],
        data: vec![
            1,0,48,0,255,255,16,0,255,255,112,0,108,0,255,255,187,220,42,181,173,60,36,199,230,65,125,124,22,8,191,157,169,169,
            85,113,83,0,79,147,213,225,127,24,199,48,221,200,55,78,28,211,84,230,134,173,195,114,9,205,40,231,223,7,61,8,212,64,
            156,21,81,42,211,136,180,255,112,43,54,224,245,21,81,14,97,192,71,63,231,247,247,138,35,155,250,190,29,223,228,80,
            108,120,155,234,213,192,204,191,136,173,124,3,52,102,118,118,89,113,107,99,71,53,105,122,113,86,78,117,77,75,121,82,
            52,67,104,67,55,119,65,98,119,101,101,55,120,71,49,68,74,87,55,68,106,68,80,90,44,67,107,116,82,117,81,50,109,116,116,
            103,82,71,107,88,74,116,121,107,115,100,75,72,106,85,100,99,50,67,52,84,103,68,122,121,66,57,56,111,69,122,121,56,44,
            53,48,48,48,48,44,80,111,111,114,105,97,71,71,32,240,159,164,169
        ]
    };

    let withdraw_spl_tokens_using_ed25519 = create_withdraw_spl_tokens_using_ed25519_signature(
        &mint_account,
        &bank_account,
        &bank_account_token_account,
        &to.pubkey(),
        &to.pubkey(),
        &to_token_account,
        &TOKEN_STANDARD_PROGRAM,
        &SYSTEM_PROGRAM_ID,
        Some(&MEMO_PROGRAM_ID),
        &program_id
    );

    let tx = Transaction::new_signed_with_payer(
        &[
            ed25519_signature_verification_instruction.clone(),
            withdraw_spl_tokens_using_ed25519.clone(),
            ed25519_signature_verification_instruction.clone(),
            withdraw_spl_tokens_using_ed25519.clone()
        ],
        Some(&payer.pubkey()),
        &[
            &payer,
            &to
        ],
        recent_blockhash
    );

    banks_client
        .process_transaction(tx)
        .await
        .unwrap();

    let bank_account_data = banks_client
        .get_account(bank_account)
        .await
        .unwrap()
        .unwrap()
        .data;
    let bank_account_info = UserBankAccount::try_from_slice(
        &bank_account_data
    ).unwrap();

    let expected_signature: [u8; 64] = ed25519_signature_verification_instruction
        .data
        .get(48..112)
        .unwrap()
        .try_into()
        .unwrap();

    assert!(!bank_account_info.signatures[0].is_ok);
    assert!(!bank_account_info.signatures[1].is_ok);

    let to_token_account_data = banks_client
        .get_account(to_token_account)
        .await
        .unwrap()
        .unwrap()
        .data;
    let to_token_account_info = TokenAccount::unpack(&to_token_account_data).unwrap();

    let bank_account_tokne_account_data = banks_client
        .get_account(bank_account_token_account)
        .await
        .unwrap()
        .unwrap()
        .data;
    let bank_account_token_account_info = TokenAccount::unpack(&bank_account_tokne_account_data).unwrap();

    assert_eq!(
        to_token_account_info.amount,
        300_00u64
    );

    assert_eq!(
        bank_account_token_account_info.amount,
        200_00u64
    );
}