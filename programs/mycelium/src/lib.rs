use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    metadata::{
        create_master_edition_v3, create_metadata_accounts_v3, CreateMasterEditionV3,
        CreateMetadataAccountsV3,
        mpl_token_metadata::types::DataV2,
    },
    token::{mint_to, Mint, MintTo, Token, TokenAccount, transfer, Transfer},
};
use anchor_spl::metadata::mpl_token_metadata::types::Creator;
use anchor_spl::metadata::mpl_token_metadata::accounts::Metadata;
use mpl_token_metadata::pda::{find_master_edition_account, find_metadata_account};

declare_id!("5TH8JUgMfbngDCAvhyxAeCFwwstZXg4x3Zi9nAYH64TY");
const CREATOR: &str = "58V6myLoy5EVJA3U2wPdRDMUXpkwg8Vfw5b6fHqi2mEj";
const SUPPLY: u64 = 6000;
const REWARD: u64 = 1000;
const PRICE: u64 = 100000;
#[program]
pub mod mycelium {

    use super::*;
    pub fn initialize2(ctx: Context<Initialize2>) -> Result<()> {
        ctx.accounts.mint_data.mint_num = 0;
        ctx.accounts.mint_data.mint_price = 1;
        ctx.accounts.stake_data.stake_num = 0;
        ctx.accounts.stake_data.stake_reward = REWARD;
        Ok(())
    }
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
    pub fn initialize_user(_ctx: Context<InitializeUser>) -> Result<()> {
        Ok(())
    }
    pub fn fund(ctx: Context<Fund>, amount: u64) -> Result<()> {
        transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    to: ctx.accounts.bank.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                }
            ),
            amount
        )?;
        Ok(())
    }
    pub fn stake(ctx: Context<Stake>) -> Result<()> {
        // let stake_data_account = &ctx.accounts.stake_data;
        // let data: &[u8] = &stake_data_account.try_borrow_data()?;
        // let mut stake_data: StakeData = StakeData::try_from_slice(data)?;
        let metadata_full_account =  match Metadata::try_from(&ctx.accounts.nft_metadata_account).ok() {
            None => return Err(CustomError::InvalidAccount.into()),
            Some(account) => account
        };
        let creators = match metadata_full_account.creators {
            None => return Err(CustomError::InvalidAccount.into()),
            Some(account) => account,
        };
        
        let mut valid = false;
        for creator in creators {
            if creator.verified && creator.address == ctx.accounts.program_authority.key() {
                valid = true;
                break;
            }
        }
        if !valid {
            return Err(CustomError::InvalidAccount.into());
        }
        transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.nft_account.to_account_info(),
                    to: ctx.accounts.stake_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                }
            ),
            1
        )?;
        let time = Clock::get()?.unix_timestamp;
        let new_size = StakeInfo::space(ctx.accounts.stake_info.mints.len() + 1);
        let lamports_required = Rent::get()?.minimum_balance(new_size);
        let stake_info = ctx.accounts.stake_info.to_account_info();
        if stake_info.lamports() < lamports_required {
            let lamports_to_transfer = lamports_required - stake_info.lamports();
            anchor_lang::solana_program::program::invoke(
                &anchor_lang::solana_program::system_instruction::transfer(
                    &ctx.accounts.user.key(),
                    stake_info.key,
                    lamports_to_transfer
                ),
                &[
                    ctx.accounts.user.to_account_info(),
                    stake_info.clone(),
                    ctx.accounts.system_program.to_account_info().clone()
                ]
            )?;
        }
        stake_info.realloc(new_size, false)?;
        ctx.accounts.stake_info.add_stake(ctx.accounts.nft_account.mint.key(), time);

        // stake_data.stake_num += 1;
        // stake_data.stake_reward = (1 - SUPPLY / stake_data.stake_num) * REWARD;
        Ok(())
    }
    pub fn unstake(ctx: Context<Unstake>) -> Result<()> {
        transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.stake_account.to_account_info(),
                    to: ctx.accounts.nft_account.to_account_info(),
                    authority: ctx.accounts.program_authority.to_account_info(),
                },
                &[&[b"auth", &[ctx.bumps.program_authority]]]
            ),
            1
        )?;
        let index = match ctx.accounts.stake_info.mints.iter().position(|&x| x == ctx.accounts.nft_account.mint) {
            None => return Err(CustomError::NotStaked.into()),
            Some(index) => index 
        };
        ctx.accounts.stake_info.remove_stake(index);
        let new_size = StakeInfo::space(ctx.accounts.stake_info.mints.len());
        ctx.accounts.stake_info.to_account_info().realloc(new_size, false)?;
        // ctx.accounts.stake_data.stake_num -= 1;
        // ctx.accounts.stake_data.stake_reward = (1 - SUPPLY / ctx.accounts.stake_data.stake_num) * REWARD;
        Ok(())
    }
    pub fn claim(ctx: Context<Claim>, amount: u64) -> Result<()> {
        if amount > 0 {
            transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.user_storage_account.to_account_info(),
                        to: ctx.accounts.user_token_account.to_account_info(),
                        authority: ctx.accounts.program_authority.to_account_info(),
                    },
                    &[&[b"auth", &[ctx.bumps.program_authority]]]
                ),
                amount
            )?;
        }
        Ok(())
    }
    pub fn crank(ctx: Context<Crank>) -> Result<()> {
        let mut total: u64 = 0;
        let date = Clock::get()?.unix_timestamp;
        let reward: u64 = 1; //ctx.accounts.stake_data.stake_reward;
        for i in 0..ctx.accounts.stake_info.mints.len() {
            total += reward * ((date - ctx.accounts.stake_info.staked_times[i]) as u64);
            ctx.accounts.stake_info.staked_times[i] = date;
        }
        total = total * 10_u64.pow(9) / 86400;
        if total > 0 {
            transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.bank.to_account_info(),
                        to: ctx.accounts.user_storage_account.to_account_info(),
                        authority: ctx.accounts.bank.to_account_info(),
                    },
                    &[&[b"bank", &[ctx.bumps.bank]]]
                ),
                total
            )?;
        }
        Ok(())
    }
    pub fn mint_nft(ctx: Context<MintNFT>) -> Result<()> {
        // transfer(
        //     CpiContext::new(
        //         ctx.accounts.token_program.to_account_info(),
        //         Transfer {
        //             from: ctx.accounts.user_token_account.to_account_info(),
        //             to: ctx.accounts.bank.to_account_info(),
        //             authority: ctx.accounts.signer.to_account_info(),
        //         }
        //     ),
        //     ctx.accounts.mint_data.mint_price
        // )?;
        // ctx.accounts.mint_data.mint_num += 1;
        // ctx.accounts.mint_data.mint_price = (SUPPLY / (SUPPLY - ctx.accounts.mint_data.mint_num)) * PRICE;
        mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.associated_token_account.to_account_info(),
                    authority: ctx.accounts.program_authority.to_account_info(),
                },
                &[&[b"auth", &[ctx.bumps.program_authority]]]
            ), 
            1
        )?;
        // create metadata account

        let data_v2 = DataV2 {
            name: String::from("Spore"),
            symbol: String::from("SPORE"),
            uri: String::from("https://www.example.com"),
            seller_fee_basis_points: 500,
            creators: Some(vec![
                Creator {
                    address: ctx.accounts.program_authority.key(),
                    verified: true,
                    share: 0,
                },
                Creator {
                    address: CREATOR.parse::<Pubkey>().unwrap(),
                    verified: false,
                    share: 100,
                }
            ]),
            collection: None,
            uses: None,
        };

        create_metadata_accounts_v3(
            CpiContext::new_with_signer(
                ctx.accounts.token_metadata_program.to_account_info(),
                CreateMetadataAccountsV3 {
                    metadata: ctx.accounts.metadata_account.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    mint_authority: ctx.accounts.program_authority.to_account_info(),
                    update_authority: ctx.accounts.program_authority.to_account_info(),
                    payer: ctx.accounts.signer.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
                &[&[b"auth", &[ctx.bumps.program_authority]]]
            ), 
            data_v2, 
            false, 
            true, 
            None
        )?;
        //create master edition account
        create_master_edition_v3(
            CpiContext::new_with_signer(
                ctx.accounts.token_metadata_program.to_account_info(),
                CreateMasterEditionV3 {
                    edition: ctx.accounts.master_edition_account.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    update_authority: ctx.accounts.program_authority.to_account_info(),
                    mint_authority: ctx.accounts.program_authority.to_account_info(),
                    payer: ctx.accounts.signer.to_account_info(),
                    metadata: ctx.accounts.metadata_account.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
                &[&[b"auth", &[ctx.bumps.program_authority]]]
            ), 
            Some(1)
        )?;
        Ok(())
    }
}
#[error_code]
pub enum CustomError {
    #[msg("Invalid Account")]
    InvalidAccount,
    #[msg("Not Staked")]
    NotStaked,
    #[msg("Out of Range")]
    OutOfRange
}
#[account]
pub struct StakeData {
    stake_num: u64,
    stake_reward: u64,
}
#[account]
pub struct MintData {
    mint_num: u64,
    mint_price: u64,
}
#[derive(Accounts)]
pub struct Initialize2<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        init,
        payer = user,
        seeds = [b"stake_data"],
        bump,
        space = 8 + 8 + 8,
    )]
    pub stake_data: Account<'info, StakeData>, 
    #[account(
        init,
        payer = user,
        seeds = [b"mint_data"],
        bump,
        space = 8 + 8 + 8,
    )]
    pub mint_data: Account<'info, MintData>,
    pub system_program: Program<'info, System>,
}
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = user,
        seeds = [b"auth"],
        bump,
        space = 8,
    )]
    /// CHECK: 
    pub program_authority: AccountInfo<'info>,
    #[account(
        init,
        payer = user,
        seeds = [b"bank"],
        bump,
        token::mint = mint,
        token::authority = bank,
    )]
    pub bank: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}
#[derive(Accounts)]
pub struct InitializeUser<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        init,
        seeds = [b"stake", user.key().as_ref()],
        bump,
        payer = user,
        space = 8 + 32 + 4 + 4
    )]
    pub stake_info: Account<'info, StakeInfo>,
    #[account(
        init,
        seeds = [b"account", user.key().as_ref()],
        bump,
        payer = user,
        token::mint = mint,
        token::authority = program_authority,
    )]
    pub user_storage_account: Account<'info, TokenAccount>,
    #[account(
        seeds = [b"bank"],
        bump
    )]
    pub bank: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(
        seeds = [b"auth"],
        bump
    )]
    /// CHECK: 
    pub program_authority: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}
#[derive(Accounts)]
pub struct Fund<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        seeds = [b"bank"],
        bump,
    )]
    pub bank: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}
#[account]
pub struct StakeInfo {
    mints: Vec<Pubkey>,
    staked_times: Vec<i64>,
}
impl StakeInfo {
    pub fn space(size: usize) -> usize {
        8 + 4 + 32 * size + 4 + 8 * size
    }
    pub fn add_stake(&mut self, mint: Pubkey, time: i64) {
        self.mints.push(mint);
        self.staked_times.push(time);
    }
    pub fn remove_stake(&mut self, i: usize) {
        self.mints.remove(i);
        self.staked_times.remove(i);
    }
}
#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    // #[account(
    //     mut,
    //     seeds = [b"stake_data"],
    //     bump,
    // )]
    // pub stake_data: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [b"stake", user.key().as_ref()],
        bump,
    )]
    pub stake_info: Account<'info, StakeInfo>,
    #[account(
        init,
        seeds = [b"stake_account", user.key().as_ref(), nft_account.key().as_ref()],
        bump,
        payer = user,
        token::mint = nft_mint,
        token::authority = program_authority
    )]
    pub stake_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = nft_account.owner == user.key(),
        constraint = nft_account.amount == 1,
        constraint = nft_account.mint == nft_mint.key()
    )]
    pub nft_account: Account<'info, TokenAccount>,
    pub nft_mint: Account<'info, Mint>,
    #[account(
        mut,
        address=find_metadata_account(&nft_mint.key()).0,
    )]
    /// CHECK: 
    pub nft_metadata_account: AccountInfo<'info>,
    #[account(
        seeds = [b"auth"],
        bump,
    )]
    /// CHECK: 
    pub program_authority: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    #[account(address = mpl_token_metadata::ID)]
    /// CHECK: 
    pub token_metadata_program: AccountInfo<'info>,
}
#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    // #[account(
    //     mut,
    //     seeds = [b"stake_data"],
    //     bump
    // )]
    // pub stake_data: Account<'info, StakeData>,
    #[account(
        mut,
        seeds = [b"stake", user.key().as_ref()],
        bump,
    )]
    pub stake_info: Account<'info, StakeInfo>,
    #[account(
        mut,
        seeds = [b"stake_account", user.key().as_ref(), nft_account.key().as_ref()],
        bump,
        constraint = stake_account.amount == 1
    )]
    pub stake_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = nft_account.amount == 0,
    )]
    pub nft_account: Account<'info, TokenAccount>,
    #[account(
        seeds = [b"auth"],
        bump,
    )]
    /// CHECK: 
    pub program_authority: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
}
#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [b"account", user.key().as_ref()],
        bump,
    )]
    pub user_storage_account: Account<'info, TokenAccount>,
    #[account(
        seeds = [b"auth"],
        bump
    )]
    /// CHECK: 
    pub program_authority: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
}
#[derive(Accounts)]
pub struct MintNFT<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    // #[account(
    //     mut, 
    //     seeds = [b"mint_data"],
    //     bump,
    // )]
    // pub mint_data: Account<'info, MintData>,
    #[account(
        seeds = [b"auth"],
        bump,
    )]
    /// CHECK: 
    pub program_authority: AccountInfo<'info>,
    #[account(
        init,
        payer = signer,
        mint::decimals = 0,
        mint::authority = program_authority.key(),
        mint::freeze_authority = program_authority.key(),
    )]
    pub mint: Account<'info, Mint>,
    #[account(
        init_if_needed,
        payer = signer,
        associated_token::mint = mint,
        associated_token::authority = signer,
    )]
    pub associated_token_account: Account<'info, TokenAccount>,
    /// CHECK - address
    #[account(
        mut,
        address=find_metadata_account(&mint.key()).0,
    )]
    pub metadata_account: AccountInfo<'info>, 
    /// CHECK: address
    #[account(
        mut,
        address=find_master_edition_account(&mint.key()).0,
    )]
    pub master_edition_account: AccountInfo<'info>, 
    // #[account(
    //     mut,
    //     seeds = [b"bank"],
    //     bump,
    // )]
    // pub bank: Account<'info, TokenAccount>,
    // #[account(mut)]
    // pub user_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    /// CHECK: 
    pub token_metadata_program: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>
}

#[derive(Accounts)]
pub struct Crank<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    /// CHECK: 
    pub user: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [b"account", user.key().as_ref()],
        bump,
    )]
    pub user_storage_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [b"bank"],
        bump,
    )]
    pub bank: Account<'info, TokenAccount>,
    // #[account(
    //     seeds = [b"stake_data"],
    //     bump,
    // )]
    // pub stake_data: Account<'info, StakeData>,
    #[account(
        mut,
        seeds = [b"stake", user.key().as_ref()],
        bump
    )]
    pub stake_info: Account<'info, StakeInfo>,
    pub token_program: Program<'info, Token>,
}