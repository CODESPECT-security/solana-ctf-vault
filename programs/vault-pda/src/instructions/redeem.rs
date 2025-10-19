use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    burn, transfer_checked, Burn, Mint, TokenAccount, TokenInterface, TransferChecked,
};

use crate::state::{Vault, VaultAuthority};

#[derive(Accounts)]
pub struct Redeem<'info> {
    #[account(
        seeds = [b"vault", underlying_mint.key().as_ref()],
        bump = vault.bump,
        has_one = underlying_mint,
        has_one = vault_token_account,
    )]
    pub vault: Account<'info, Vault>,

    /// The underlying asset mint
    pub underlying_mint: InterfaceAccount<'info, Mint>,

    /// The vault's token account that holds underlying assets
    #[account(mut)]
    pub vault_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The share mint
    #[account(mut)]
    pub share_mint: InterfaceAccount<'info, Mint>,

    /// The vault authority that can transfer from vault
    #[account(
        seeds = [b"vault_authority"],
        bump = vault_authority.bump
    )]
    pub vault_authority: Account<'info, VaultAuthority>,

    /// The redeemer's token account for receiving underlying assets
    #[account(
        mut,
        token::mint = underlying_mint,
        token::authority = redeemer,
    )]
    pub redeemer_underlying_account: InterfaceAccount<'info, TokenAccount>,

    /// The redeemer's token account for burning shares
    #[account(
        mut,
        token::mint = share_mint,
        token::authority = redeemer,
    )]
    pub redeemer_share_account: InterfaceAccount<'info, TokenAccount>,

    pub redeemer: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler(ctx: Context<Redeem>, shares: u64) -> Result<()> {
    require!(shares > 0, RedeemError::InvalidAmount);

    let share_mint = &ctx.accounts.share_mint;
    let vault_token_account = &ctx.accounts.vault_token_account;

    // Prevent division by zero
    require!(share_mint.supply > 0, RedeemError::NoShares);
    require!(vault_token_account.amount > 0, RedeemError::EmptyVault);

    // Calculate underlying tokens to return: (shares * total_assets) / total_shares
    // Use u128 to prevent overflow during multiplication
    let underlying_to_return = (shares as u128)
        .checked_mul(vault_token_account.amount as u128)
        .ok_or(RedeemError::MathOverflow)?
        .checked_div(share_mint.supply as u128)
        .ok_or(RedeemError::MathOverflow)?;

    let underlying_to_return = underlying_to_return as u64;

    require!(underlying_to_return > 0, RedeemError::InsufficientUnderlying);

    // Burn shares from redeemer
    let burn_accounts = Burn {
        mint: ctx.accounts.share_mint.to_account_info(),
        from: ctx.accounts.redeemer_share_account.to_account_info(),
        authority: ctx.accounts.redeemer.to_account_info(),
    };

    let cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        burn_accounts,
    );

    burn(cpi_ctx, shares)?;

    // Transfer underlying tokens from vault to redeemer
    let vault_authority_bump = ctx.accounts.vault_authority.bump;
    let vault_authority_seeds = &[b"vault_authority".as_ref(), &[vault_authority_bump]];
    let signer_seeds = &[&vault_authority_seeds[..]];

    let transfer_accounts = TransferChecked {
        from: ctx.accounts.vault_token_account.to_account_info(),
        mint: ctx.accounts.underlying_mint.to_account_info(),
        to: ctx.accounts.redeemer_underlying_account.to_account_info(),
        authority: ctx.accounts.vault_authority.to_account_info(),
    };

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        transfer_accounts,
        signer_seeds,
    );

    transfer_checked(cpi_ctx, underlying_to_return, ctx.accounts.underlying_mint.decimals)?;

    msg!("Redeem successful!");
    msg!("Shares burned: {}", shares);
    msg!("Underlying returned: {}", underlying_to_return);
    msg!("Remaining vault assets: {}", vault_token_account.amount - underlying_to_return);
    msg!("Remaining shares supply: {}", share_mint.supply - shares);

    Ok(())
}

#[error_code]
pub enum RedeemError {
    #[msg("Shares amount must be greater than zero")]
    InvalidAmount,
    #[msg("No shares exist in circulation")]
    NoShares,
    #[msg("Vault has no assets")]
    EmptyVault,
    #[msg("Math operation overflow")]
    MathOverflow,
    #[msg("Insufficient underlying tokens would be returned")]
    InsufficientUnderlying,
}
