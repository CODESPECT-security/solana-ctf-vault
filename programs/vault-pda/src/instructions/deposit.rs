use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    mint_to, transfer_checked, Mint, MintTo, TokenAccount, TokenInterface, TransferChecked,
};

use crate::state::{Vault, VaultAuthority};

#[derive(Accounts)]
pub struct Deposit<'info> {
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

    /// The vault authority that can mint shares
    #[account(
        seeds = [b"vault_authority"],
        bump = vault_authority.bump
    )]
    pub vault_authority: Account<'info, VaultAuthority>,

    /// The depositor's token account for the underlying asset
    #[account(
        mut,
        token::mint = underlying_mint,
        token::authority = depositor,
    )]
    pub depositor_underlying_account: InterfaceAccount<'info, TokenAccount>,

    /// The depositor's token account for receiving shares
    #[account(
        mut,
        token::mint = share_mint,
        token::authority = depositor,
    )]
    pub depositor_share_account: InterfaceAccount<'info, TokenAccount>,

    pub depositor: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler(ctx: Context<Deposit>, amount: u64) -> Result<()> {
    require!(amount > 0, DepositError::InvalidAmount);

    // Validate that the share_mint matches the vault's share_mint
    require!(
        ctx.accounts.share_mint.key() == ctx.accounts.vault.share_mint,
        DepositError::InvalidShareMint
    );

    let share_mint = &ctx.accounts.share_mint;
    let vault_token_account = &ctx.accounts.vault_token_account;

    // Calculate shares to mint based on vault state
    let shares_to_mint = if share_mint.supply == 0 {
        // First deposit: mint shares 1:1 with deposited amount
        amount
    } else {
        // Subsequent deposits: shares = (amount * total_shares) / total_assets
        let total_shares = share_mint.supply;
        let total_assets = vault_token_account.amount;

        // Prevent division by zero (should not happen, but safety check)
        require!(total_assets > 0, DepositError::InvalidVaultState);

        // Calculate: (amount * total_shares) / total_assets
        // Use u128 to prevent overflow during multiplication
        let shares = (amount as u128)
            .checked_mul(total_shares as u128)
            .ok_or(DepositError::MathOverflow)?
            .checked_div(total_assets as u128)
            .ok_or(DepositError::MathOverflow)?;

        shares as u64
    };

    require!(shares_to_mint > 0, DepositError::InsufficientShares);

    // Transfer underlying tokens from depositor to vault
    let transfer_accounts = TransferChecked {
        from: ctx.accounts.depositor_underlying_account.to_account_info(),
        mint: ctx.accounts.underlying_mint.to_account_info(),
        to: ctx.accounts.vault_token_account.to_account_info(),
        authority: ctx.accounts.depositor.to_account_info(),
    };

    let cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        transfer_accounts,
    );

    transfer_checked(cpi_ctx, amount, ctx.accounts.underlying_mint.decimals)?;

    // Mint shares to depositor
    let vault_authority_bump = ctx.accounts.vault_authority.bump;
    let vault_authority_seeds = &[b"vault_authority".as_ref(), &[vault_authority_bump]];
    let signer_seeds = &[&vault_authority_seeds[..]];

    let mint_accounts = MintTo {
        mint: ctx.accounts.share_mint.to_account_info(),
        to: ctx.accounts.depositor_share_account.to_account_info(),
        authority: ctx.accounts.vault_authority.to_account_info(),
    };

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        mint_accounts,
        signer_seeds,
    );

    mint_to(cpi_ctx, shares_to_mint)?;

    msg!("Deposit successful!");
    msg!("Deposited: {} tokens", amount);
    msg!("Minted: {} shares", shares_to_mint);
    msg!("Total vault assets: {}", vault_token_account.amount + amount);
    msg!("Total shares supply: {}", share_mint.supply + shares_to_mint);

    Ok(())
}

#[error_code]
pub enum DepositError {
    #[msg("Deposit amount must be greater than zero")]
    InvalidAmount,
    #[msg("Vault state is invalid")]
    InvalidVaultState,
    #[msg("Math operation overflow")]
    MathOverflow,
    #[msg("Insufficient shares would be minted")]
    InsufficientShares,
    #[msg("Share mint does not match vault's share mint")]
    InvalidShareMint,
}
