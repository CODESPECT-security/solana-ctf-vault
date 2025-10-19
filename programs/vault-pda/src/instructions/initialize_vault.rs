use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::state::{Vault, VaultAuthority};

#[derive(Accounts)]
pub struct InitializeVault<'info> {
    #[account(
        init,
        payer = payer,
        space = Vault::LEN,
        seeds = [b"vault", underlying_mint.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, Vault>,

    /// The underlying asset mint that the vault will hold
    pub underlying_mint: InterfaceAccount<'info, Mint>,

    /// The token account that will hold the vault's underlying assets
    #[account(
        init,
        payer = payer,
        token::mint = underlying_mint,
        token::authority = vault_authority,
        token::token_program = token_program,
        seeds = [b"vault_token_account", vault.key().as_ref()],
        bump
    )]
    pub vault_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The share mint account to be created
    /// This will be initialized in the instruction with vault_authority as mint authority
    #[account(
        init,
        payer = payer,
        mint::decimals = underlying_mint.decimals,
        mint::authority = vault_authority,
        mint::token_program = token_program,
        seeds = [b"share_mint", vault.key().as_ref()],
        bump
    )]
    pub share_mint: InterfaceAccount<'info, Mint>,

    /// The vault_authority PDA that serves as the mint authority for shares
    /// Must be initialized via the initialize instruction first
    #[account(
        seeds = [b"vault_authority"],
        bump = vault_authority.bump
    )]
    pub vault_authority: Account<'info, VaultAuthority>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler(ctx: Context<InitializeVault>) -> Result<()> {
    let vault = &mut ctx.accounts.vault;

    vault.share_mint = ctx.accounts.share_mint.key();
    vault.underlying_mint = ctx.accounts.underlying_mint.key();
    vault.vault_token_account = ctx.accounts.vault_token_account.key();
    vault.bump = ctx.bumps.vault;

    msg!("Vault initialized successfully!");
    msg!("Vault: {}", vault.key());
    msg!("Share Mint: {}", vault.share_mint);
    msg!("Underlying Mint: {}", vault.underlying_mint);
    msg!("Vault Token Account: {}", vault.vault_token_account);
    msg!("Vault Authority: {}", ctx.accounts.vault_authority.key());

    Ok(())
}
