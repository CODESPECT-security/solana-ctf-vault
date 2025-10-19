use anchor_lang::prelude::*;

use crate::state::ProtocolState;

#[derive(Accounts)]
pub struct TransferOwnership<'info> {
    #[account(
        mut,
        seeds = [b"protocol_state"],
        bump = protocol_state.bump,
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// CHECK: Current protocol owner
    pub current_owner: UncheckedAccount<'info>,

    /// CHECK: New protocol owner
    pub new_owner: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<TransferOwnership>) -> Result<()> {
    let protocol_state = &mut ctx.accounts.protocol_state;

    require!(
        ctx.accounts.current_owner.key() == protocol_state.owner,
        TransferOwnershipError::Unauthorized
    );

    protocol_state.owner = ctx.accounts.new_owner.key();

    msg!("Ownership transferred!");
    msg!("Previous owner: {}", ctx.accounts.current_owner.key());
    msg!("New owner: {}", ctx.accounts.new_owner.key());

    Ok(())
}

#[error_code]
pub enum TransferOwnershipError {
    #[msg("Only the current owner can transfer ownership")]
    Unauthorized,
}
