use anchor_lang::prelude::*;

#[account]
pub struct ProtocolState {
    /// The protocol owner who can perform administrative actions
    pub owner: Pubkey,
    /// Bump seed for PDA derivation
    pub bump: u8,
}

impl ProtocolState {
    pub const LEN: usize = 8 + // discriminator
        32 + // owner
        1; // bump
}
