# Task

```
Write an anchor smart contract that allows wallets to create a USDC (or any SPL token) vault, transfer tokens to it, and withdraw from it. 

Write an accompanying rust service that crawls the user vaults once per month and transfers 1% of the currently staked amount to the vault in interest.

If possible:
- use latest anchor packages
- use async rust functions in the service (solana_client::non_blocking)

The interest deposit should be a permission-less instruction where the check for how much amount should be transferred should be in the program logic so that any client can call the crank.

```

## Project Structure
- Anchor program: `programs/gfx-task`
- TypeScript test: `tests/gfx-task.ts`
- Rust client (Crank caller): `gfx-task-client`

## Instructions
1. `initialize_treasury`: Source of tokens for paying interest.
2. `initialize_vault`: Set up a vault for user token deposits.
3. `deposit`: User deposits tokens.
4. `pay_interest`: Pays interest.

### How to Run
1. `anchor build` and update the new program ID in `Anchor.toml`, `programs/gfx-task/src/lib.rs` (`declare_id!()`) and `gfx-task-client/src/main.rs` (`GFX_TASK_PROGRAM_ID`)
2. Start local validator, `solana-test-validator`
3. `anchor test --skip-local-validator` - will create a new SPL token for each run, create a vault, deposit tokens and pays interest (if specified).
4. `cargo run` (in `gfx-task-client/`) - will call the crank using the rust client

Repeating step 2 without creating new vaults, it will return an error for each vault since the interest for the current month has already been paid.

The ts tests can also call the crank if called with `INCLUDE_PAY_INTEREST=true anchor test --skip-local-validator`.


### Simplification
For simplification, each month is assumed to be 30 days. A new month for interest calculations starts with each successful crank execution.
