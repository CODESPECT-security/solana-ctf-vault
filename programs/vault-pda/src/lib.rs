pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("8qsydpwMiRcFtJ8wrKkM4xrMMEWfnw2szibQGLgBw6KH");

#[program]
pub mod vault_pda {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        initialize::handler(ctx)
    }

    pub fn initialize_vault(ctx: Context<InitializeVault>) -> Result<()> {
        initialize_vault::handler(ctx)
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        deposit::handler(ctx, amount)
    }

    pub fn redeem(ctx: Context<Redeem>, shares: u64) -> Result<()> {
        redeem::handler(ctx, shares)
    }

    pub fn transfer_ownership(ctx: Context<TransferOwnership>) -> Result<()> {
        transfer_ownership::handler(ctx)
    }
}
