use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{transfer, Mint, Token, TokenAccount, Transfer},
};
use solana_program::{clock, pubkey};

const ADMIN_PUBKEY: Pubkey = pubkey!("4jURAvf4NbrLki15eNWuojugzENbAntJdd7NP5FFgy3q");
const MONTH_IN_SECONDS: u64 = clock::SECONDS_PER_DAY * 30;

declare_id!("Gw5zhyN2zL7Y3PjKAT2bcbPmk9LdPjkqBxtpXPfAKyLE");

#[program]
pub mod gfx_task {
    use super::*;

    pub fn initialize_treasury(ctx: Context<InitializeTreasury>, amount: u64) -> Result<()> {
        let destination = ctx.accounts.treasury_token_account.to_account_info();
        let source = ctx.accounts.admin_ata.to_account_info();
        let token_program = ctx.accounts.token_program.to_account_info();
        let authority = ctx.accounts.admin.to_account_info();

        cpi_token_transfer(destination, source, token_program, authority, amount)
    }

    pub fn initialize_vault(ctx: Context<InitializeVault>) -> Result<()> {
        ctx.accounts.vault.user = ctx.accounts.user.key();
        ctx.accounts.vault.mint = ctx.accounts.mint.key();
        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        // Set up references to the accounts involved in the transaction
        let destination = ctx.accounts.vault_token_account.to_account_info();
        let source = ctx.accounts.from_ata.to_account_info();
        let token_program = ctx.accounts.token_program.to_account_info();
        let authority = ctx.accounts.from.to_account_info();

        cpi_token_transfer(destination, source, token_program, authority, amount)
    }

    pub fn pay_interest(ctx: Context<PayInterest>, bump: u8) -> Result<()> {
        let clock = Clock::get()?;
        let seconds_since_last_payment =
            clock.unix_timestamp - ctx.accounts.vault.last_interest_payment_timestamp;

        if seconds_since_last_payment < MONTH_IN_SECONDS.try_into().unwrap() {
            return err!(GfxTaskError::InterestAlreadyPaid);
        }

        let amount = &ctx.accounts.vault_token_account.amount / 100;
        let destination = &ctx.accounts.vault_token_account;
        let source = &ctx.accounts.treasury_token_account;
        let token_program = &ctx.accounts.token_program;
        let authority = &ctx.accounts.treasury_authority;

        let mint_key = *ctx.accounts.mint.to_account_info().key;
        let treasury_authority_seeds = &[b"gfx_task_treasury", mint_key.as_ref(), &[bump]];
        let treasury_authority_signer = &[&treasury_authority_seeds[..]];

        let cpi_accounts = Transfer {
            from: source.to_account_info(),
            to: destination.to_account_info(),
            authority: authority.to_account_info(),
        };
        let cpi_program = token_program.to_account_info();
        let cpi_ctx =
            CpiContext::new_with_signer(cpi_program, cpi_accounts, treasury_authority_signer);

        transfer(cpi_ctx, amount)?;

        ctx.accounts.vault.last_interest_payment_timestamp = clock.unix_timestamp;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeTreasury<'info> {
    pub mint: Account<'info, Mint>,
    #[account(
        seeds = [b"gfx_task_treasury", mint.key().as_ref()],
        bump
    )]
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub treasury_authority: AccountInfo<'info>,
    #[account(
        init,
        payer = admin,
        associated_token::mint = mint,
        associated_token::authority = treasury_authority,
    )]
    pub treasury_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = admin,
    )]
    pub admin_ata: Account<'info, TokenAccount>,
    #[account(mut, address = ADMIN_PUBKEY)]
    pub admin: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeVault<'info> {
    pub mint: Account<'info, Mint>,
    #[account(
        init,
        payer = user,
        space = 8 + 8 + 32 + 32,
        seeds = [b"gfx_task_vault", user.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, Vault>,
    #[account(
        init,
        payer = user,
        associated_token::mint = mint,
        associated_token::authority = vault,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    pub mint: Account<'info, Mint>,
    #[account(
        seeds = [b"gfx_task_vault", from.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, Vault>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = vault,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = from,
    )]
    pub from_ata: Account<'info, TokenAccount>,
    #[account(mut)]
    pub from: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PayInterest<'info> {
    pub mint: Account<'info, Mint>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub user: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [b"gfx_task_vault", user.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, Vault>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = vault,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,
    #[account(
        seeds = [b"gfx_task_treasury", mint.key().as_ref()],
        bump
    )]
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub treasury_authority: AccountInfo<'info>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = treasury_authority,
    )]
    pub treasury_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub signer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Debug)]
#[account]
pub struct Vault {
    pub last_interest_payment_timestamp: i64,
    pub user: Pubkey,
    pub mint: Pubkey,
}

#[error_code]
pub enum GfxTaskError {
    #[msg("Interest already paid for this month")]
    InterestAlreadyPaid,
}

fn cpi_token_transfer<'a>(
    to: AccountInfo<'a>,
    from: AccountInfo<'a>,
    cpi_program: AccountInfo<'a>,
    authority: AccountInfo<'a>,
    amount: u64,
) -> Result<()> {
    let cpi_accounts = Transfer {
        from,
        to,
        authority,
    };
    transfer(CpiContext::new(cpi_program, cpi_accounts), amount)
}
