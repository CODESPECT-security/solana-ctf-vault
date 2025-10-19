use anchor_lang::prelude::*;

#[account]
pub struct VaultAuthority {
    /// Bump seed for PDA derivation
    pub bump: u8,
}

impl VaultAuthority {
    pub const LEN: usize = 8 + // discriminator
        1; // bump
}
