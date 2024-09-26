use {
    num_enum::{
        IntoPrimitive,
        TryFromPrimitive
    },
    thiserror::Error
};

#[repr(u8)]
#[derive(Debug, PartialEq, Clone, Copy, TryFromPrimitive, IntoPrimitive, Error)]
pub enum BankError {
    #[error("signature already used and cannot use it twice.")]
    SignatureAlreadyUsed,
    #[error("invalid message-v1 format.")]
    MessageV1ValidationFailed,
    #[error("invalid message-v2 format.")]
    MessageV2ValidationFailed,
    #[error("invalid signature recepient pubkey.")]
    InvalidToPubkey,
    #[error("invalid amount of lamports.")]
    InvalidLamports,
    #[error("invalid mint account.")]
    InvalidMint,
    #[error("invalid spl-token amount.")]
    InvalidTokenAmount,
    #[error("insufficient lamport balance.")]
    InsufficientLamportBalance,
    #[error("failed to get ed25519 instruction")]
    FailedToGetEd25519Instruction,
    #[error("invalid ed25519 signature verification instruction")]
    InvalidEd25519SignatureVerificationInstruction,
    #[error("invalid memo program account")]
    InvalidMemoProgramAccount,
    #[error("invalid system program account")]
    InvalidSystemProgramAccount,
    #[error("invalid spl-token program account")]
    InvalidSplTokenProgramAccount,
    #[error("invalid mint account")]
    InvalidMintAccount,
    #[error("invalid associated token account for bank-account")]
    InvalidBankAssociatedTokenAccount
}