use anchor_lang::prelude::*;

use crate::state::{ProtocolState, VaultAuthority};

#[derive(Accounts)]
pub struct Initialize<'info> {
    /// The protocol state account that holds the protocol owner
    #[account(
        init,
        payer = payer,
        space = ProtocolState::LEN,
        seeds = [b"protocol_state"],
        bump
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    /// The vault authority PDA that will be used as mint/burn authority for all vaults
    #[account(
        init,
        payer = payer,
        space = VaultAuthority::LEN,
        seeds = [b"vault_authority"],
        bump
    )]
    pub vault_authority: Account<'info, VaultAuthority>,

    /// The initial protocol owner
    pub owner: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Initialize>) -> Result<()> {
    let protocol_state = &mut ctx.accounts.protocol_state;
    let vault_authority = &mut ctx.accounts.vault_authority;

    protocol_state.owner = ctx.accounts.owner.key();
    protocol_state.bump = ctx.bumps.protocol_state;

    vault_authority.bump = ctx.bumps.vault_authority;

    msg!("Protocol initialized successfully!");
    msg!("Protocol State: {}", protocol_state.key());
    msg!("Protocol Owner: {}", protocol_state.owner);
    msg!("Vault Authority: {}", vault_authority.key());

    Ok(())
}
