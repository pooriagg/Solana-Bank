use {
    solana_program::{
        entrypoint,
        entrypoint::ProgramResult,
        pubkey::Pubkey,
        account_info::AccountInfo,
        program_error::ProgramError
    },
    
    crate::processor::Processor
};

entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts_info: &[AccountInfo],
    instruction_data: &[u8]
) -> ProgramResult {
    if let Err(error) = Processor::processor(program_id, accounts_info, instruction_data) {
        return Err(error);
    };

    Ok(())
}
