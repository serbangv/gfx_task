use anchor_client::{
    solana_sdk::{
        pubkey::Pubkey,
        signature::{Keypair, Signer},
    },
    Client, Cluster, Program, ProgramAccountsIterator,
};
use anchor_spl::{
    associated_token::{
        get_associated_token_address_with_program_id, ID as associated_token_program_id,
    },
    token::ID as token_program_id,
};
use anyhow::Result;
use gfx_task::{accounts as gfx_task_accounts, instruction as gfx_task_instruction, Vault};
use solana_client::rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType};
use solana_sdk::{
    commitment_config::CommitmentConfig, pubkey, signature::read_keypair_file, system_program,
};
use std::rc::Rc;

const GFX_TASK_PROGRAM_ID: Pubkey = pubkey!("Gw5zhyN2zL7Y3PjKAT2bcbPmk9LdPjkqBxtpXPfAKyLE");

struct GfxTaskClient {
    client: Client<Rc<Keypair>>,
    payer: Rc<Keypair>,
}

impl GfxTaskClient {
    pub fn new() -> Self {
        let payer = read_keypair_file(&*shellexpand::tilde("~/.config/solana/id.json"))
            .expect("Example requires a keypair file");
        let url = Cluster::Localnet;
        let payer = Rc::new(payer);

        Self {
            client: Client::new_with_options(url, payer.clone(), CommitmentConfig::confirmed()),
            payer,
        }
    }

    pub fn program(&self) -> Program<Rc<Keypair>> {
        self.client.program(GFX_TASK_PROGRAM_ID).unwrap()
    }

    pub fn payer_pubkey(&self) -> Pubkey {
        self.payer.pubkey()
    }

    pub async fn get_vault_accounts(&self) -> ProgramAccountsIterator<Vault> {
        let vault_account_discriminator_bytes: Vec<u8> = vec![211, 8, 232, 43, 2, 152, 117, 119];
        let memcmp_filter = RpcFilterType::Memcmp(Memcmp::new(
            0,
            MemcmpEncodedBytes::Bytes(vault_account_discriminator_bytes),
        ));

        self.program()
            .accounts_lazy(vec![memcmp_filter])
            .await
            .unwrap()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let client: GfxTaskClient = GfxTaskClient::new();

    let vault_accounts_iter: ProgramAccountsIterator<Vault> = client.get_vault_accounts().await;
    let vault_accounts: Vec<_> = vault_accounts_iter.collect();

    println!("Found {} vault accounts.", &vault_accounts.len());

    for vault_account in vault_accounts {
        match vault_account {
            Ok((vault_pubkey, account)) => {
                if let Err(e) = pay_interest(
                    &client.program(),
                    account,
                    vault_pubkey,
                    client.payer_pubkey(),
                )
                .await
                {
                    eprintln!("Failed to pay interest. {}\n", e);
                }
            }
            Err(e) => {
                eprintln!("Error: {:?}", e);
            }
        }
    }

    Ok(())
}

pub async fn pay_interest(
    program: &Program<Rc<Keypair>>,
    vault_data: Vault,
    vault_pubkey: Pubkey,
    signer: Pubkey,
) -> Result<()> {
    println!("Paying interest to vault {}.", &vault_pubkey);

    let mint = vault_data.mint;
    let user = vault_data.user;
    let vault = vault_pubkey;
    let vault_token_account =
        get_associated_token_address_with_program_id(&vault_pubkey, &mint, &token_program_id);

    let treasury_authority_seeds = &[b"gfx_task_treasury".as_ref(), mint.as_ref()];
    let (treasury_authority, bump) =
        Pubkey::find_program_address(treasury_authority_seeds, &GFX_TASK_PROGRAM_ID);
    let treasury_token_account =
        get_associated_token_address_with_program_id(&treasury_authority, &mint, &token_program_id);

    program
        .request()
        .accounts(gfx_task_accounts::PayInterest {
            mint,
            user,
            vault,
            vault_token_account,
            treasury_authority,
            treasury_token_account,
            signer,
            token_program: token_program_id,
            associated_token_program: associated_token_program_id,
            system_program: system_program::ID,
        })
        .args(gfx_task_instruction::PayInterest { bump })
        .send()
        .await?;

    Ok(())
}
