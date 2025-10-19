use anchor_lang::prelude::*;

#[account]
pub struct Vault {
    /// The mint account for shares tokens (minted on deposits, burned on redeems)
    pub share_mint: Pubkey,
    /// The mint account for the underlying asset held by the vault
    pub underlying_mint: Pubkey,
    /// The token account that holds the underlying assets
    pub vault_token_account: Pubkey,
    /// Bump seed for PDA derivation
    pub bump: u8,
}

impl Vault {
    pub const LEN: usize = 8 + // discriminator
        32 + // share_mint
        32 + // underlying_mint
        32 + // vault_token_account
        1; // bump
}
