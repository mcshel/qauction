use crate::program::Qauction;
use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::token::{Token};
use anchor_spl::associated_token::{AssociatedToken};
use anchor_spl::token::{self, Mint, TokenAccount};
//use spl_token::instruction::AuthorityType::CloseAccount;

declare_id!("5EYANgeHu9DDa3WHc2td29eguhEAJ7SZFAXhWuXam2Ve");

pub const MAX_NAME_LEN: usize = 64;
pub const AUCTION_GRACE_PERIOD: i64 = 60;
pub const AUCTION_EXTENSION_PERIOD: i64 = 60;

#[program]
pub mod qauction {
    use super::*;
    
    pub fn init_admin(ctx: Context<InitAdmin>, admin_key: Pubkey) -> Result<()> {
        let admin_settings = &mut ctx.accounts.admin_settings;
        admin_settings.admin_key = admin_key;
        
        Ok(())
    }
    
    pub fn set_admin(ctx: Context<SetAdmin>, admin_key: Pubkey) -> Result<()> {
        let admin_settings = &mut ctx.accounts.admin_settings;
        admin_settings.admin_key = admin_key;
        
        Ok(())
    }

    pub fn initialize(ctx: Context<Initialize>, name: String, price: u64, price_increment: u64, start_timestamp: i64, end_timestamp: i64) -> Result<()> {
        
        let clock = Clock::get()?;
        
        require!(
            start_timestamp < end_timestamp,
            AuctionError::StartAfterEndTimestamp
        );
        
        require!(
            clock.unix_timestamp < end_timestamp,
            AuctionError::EndTimestampAlreadyPassed
        );
        
        require!(
            name.len() <= MAX_NAME_LEN,
            AuctionError::NameTooLong
        );
        
        let auction = &mut ctx.accounts.auction;
        auction.bump = *ctx.bumps.get("auction").unwrap();
        auction.name = name;
        auction.amount = price;
        auction.amount_increment = price_increment;
        auction.lamports = ctx.accounts.authority_token_account.to_account_info().lamports();
        auction.start_timestamp = start_timestamp;
        auction.end_timestamp = end_timestamp;
        auction.leader = ctx.accounts.authority.key();
        auction.leader_token_account = ctx.accounts.authority_token_account.key();
        
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.authority_token_account.to_account_info(),
                    to: ctx.accounts.proceeds.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
            ),
            price,
        )?;
        
        Ok(())
    }
    
    pub fn bid(ctx: Context<Bid>, amount: u64) -> Result<()> {
        
        let clock = Clock::get()?;
        let auction = &mut ctx.accounts.auction;
        
        require!(
            clock.unix_timestamp >= auction.start_timestamp,
            AuctionError::AuctionNotStarted
        );
        
        require!(
            clock.unix_timestamp < auction.end_timestamp,
            AuctionError::AuctionEnded
        );
        
        let amount_min: u64 = auction.amount.checked_add(auction.amount_increment).ok_or(AuctionError::InvalidCalculation)?;
        require!(
            amount_min <= amount,
            AuctionError::BidTooLow
        );
        
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.authority_token_account.to_account_info(),
                    to: ctx.accounts.proceeds.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
            ),
            amount,
        )?;
        
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.authority.to_account_info(),
                    to: ctx.accounts.leader.to_account_info(),
                },
            ),
            auction.lamports,
        )?;
        
        
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.proceeds.to_account_info(),
                    to: ctx.accounts.leader_token_account.to_account_info(),
                    authority: auction.to_account_info(),
                },
                &[&[b"auction".as_ref(), auction.name.as_bytes().as_ref(), &[auction.bump]]],
            ),
            auction.amount,
        )?;
        
        
        auction.amount = amount;
        auction.leader = ctx.accounts.authority.key();
        auction.leader_token_account = ctx.accounts.authority_token_account.key();
        if auction.end_timestamp - clock.unix_timestamp < AUCTION_GRACE_PERIOD {
            auction.end_timestamp = clock.unix_timestamp.checked_add(AUCTION_EXTENSION_PERIOD).ok_or(AuctionError::InvalidCalculation)?;
        }
        
        Ok(())
    }
    
    pub fn bid_create(ctx: Context<BidCreate>, amount: u64) -> Result<()> {
        
        let clock = Clock::get()?;
        let auction = &mut ctx.accounts.auction;
        
        require!(
            clock.unix_timestamp >= auction.start_timestamp,
            AuctionError::AuctionNotStarted
        );
        
        require!(
            clock.unix_timestamp < auction.end_timestamp,
            AuctionError::AuctionEnded
        );
        
        let amount_min: u64 = auction.amount.checked_add(auction.amount_increment).ok_or(AuctionError::InvalidCalculation)?;
        require!(
            amount_min <= amount,
            AuctionError::BidTooLow
        );
        
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.authority_token_account.to_account_info(),
                    to: ctx.accounts.proceeds.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
            ),
            amount,
        )?;
        
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.proceeds.to_account_info(),
                    to: ctx.accounts.leader_token_account.to_account_info(),
                    authority: auction.to_account_info(),
                },
                &[&[b"auction".as_ref(), auction.name.as_bytes().as_ref(), &[auction.bump]]],
            ),
            auction.amount,
        )?;
    
        
        auction.amount = amount;
        auction.leader = ctx.accounts.authority.key();
        auction.leader_token_account = ctx.accounts.authority_token_account.key();
        if auction.end_timestamp - clock.unix_timestamp < 60 {
            auction.end_timestamp = clock.unix_timestamp + 60;
        }
        
        Ok(())
    }
    
     pub fn close(ctx: Context<Close>) -> Result<()> {
        let clock = Clock::get()?;
        let auction = &ctx.accounts.auction;
        
        msg!("{}, {}", clock.unix_timestamp, auction.end_timestamp);
        
        require!(
            clock.unix_timestamp > auction.end_timestamp,
            AuctionError::AuctionNotFinished
        );
        
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.proceeds.to_account_info(),
                    to: ctx.accounts.authority_token_account.to_account_info(),
                    authority: ctx.accounts.auction.to_account_info(),
                },
                &[&[b"auction".as_ref(), auction.name.as_bytes().as_ref(), &[auction.bump]]],
            ),
            auction.amount,
        )?;
        
        token::close_account(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::CloseAccount {
                    account: ctx.accounts.proceeds.to_account_info(),
                    destination: ctx.accounts.leader.to_account_info(),
                    authority: ctx.accounts.auction.to_account_info(),
                },
                &[&[b"auction".as_ref(), auction.name.as_bytes().as_ref(), &[auction.bump]]],
            )
        )?;

        Ok(())
    }
}


#[derive(Accounts)]
pub struct InitAdmin<'info> {
    #[account(
        init,
        seeds = [b"admin".as_ref()], 
        bump, 
        payer = authority,
        space = 8 + 32,
    )]
    pub admin_settings: Account<'info, AdminSettings>,
    #[account(constraint = program.programdata_address()? == Some(program_data.key()))]
    pub program: Program<'info, Qauction>,
    #[account(constraint = program_data.upgrade_authority_address == Some(authority.key()))]
    pub program_data: Account<'info, ProgramData>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetAdmin<'info> {
    #[account(mut, seeds = [b"admin".as_ref()], bump)]
    pub admin_settings: Account<'info, AdminSettings>,
    #[account(constraint = program.programdata_address()? == Some(program_data.key()))]
    pub program: Program<'info, Qauction>,
    #[account(constraint = program_data.upgrade_authority_address == Some(authority.key()))]
    pub program_data: Account<'info, ProgramData>,
    #[account(mut)]
    pub authority: Signer<'info>,
    
}

#[derive(Accounts)]
#[instruction(name: String)]
pub struct Initialize<'info> {
    #[account(seeds = [b"admin".as_ref()], bump)]
    pub admin_settings: Account<'info, AdminSettings>,
    #[account(
        init, 
        seeds = [b"auction".as_ref(), name.as_bytes().as_ref()],
        bump, 
        payer = authority, 
        space = 8 + 1 + MAX_NAME_LEN + 8 + 8 + 8 + 8 + 8 + 32 + 32,
    )]
    pub auction: Account<'info, Auction>,
    #[account(
        init,
        seeds = [b"proceeds".as_ref(), auction.key().as_ref()],
        bump,
        payer = authority,
        token::mint = proceeds_mint,
        token::authority = auction,
    )]
    pub proceeds: Account<'info, TokenAccount>,
    pub proceeds_mint: Account<'info, Mint>,
    #[account(
        mut,
        associated_token::mint = proceeds_mint,
        associated_token::authority = authority
    )]
    pub authority_token_account: Account<'info, TokenAccount>,
    #[account(mut, constraint = admin_settings.admin_key == authority.key())]
    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Bid<'info> {
    #[account(mut)]
    pub auction: Box<Account<'info, Auction>>,
    #[account(
        mut,
        seeds = [b"proceeds".as_ref(), auction.key().as_ref()],
        bump,
    )]
    pub proceeds: Account<'info, TokenAccount>,
    pub proceeds_mint: Account<'info, Mint>,
    #[account(
        mut, 
        constraint = auction.leader_token_account == leader_token_account.key()
    )]
    pub leader_token_account: Account<'info, TokenAccount>,
    /// CHECKED: custom constraint ensures the validity of this account
    #[account(
        mut, 
        constraint = auction.leader == leader.key()
    )]
    pub leader: UncheckedAccount<'info>,
    #[account(
        mut,
        associated_token::mint = proceeds_mint,
        associated_token::authority = authority,
    )]
    pub authority_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BidCreate<'info> {
    #[account(mut)]
    pub auction: Box<Account<'info, Auction>>,
    #[account(
        mut,
        seeds = [b"proceeds".as_ref(), auction.key().as_ref()],
        bump,
    )]
    pub proceeds: Account<'info, TokenAccount>,
    pub proceeds_mint: Account<'info, Mint>,
    #[account(
        init,
        payer = authority,
        associated_token::mint = proceeds_mint,
        associated_token::authority = leader,
    )]
    pub leader_token_account: Account<'info, TokenAccount>,
    /// CHECKED: custom constraint ensures the validity of this account
    #[account(
        mut, 
        constraint = auction.leader == leader.key()
    )]
    pub leader: UncheckedAccount<'info>,
    #[account(
        mut,
        associated_token::mint = proceeds_mint,
        associated_token::authority = authority,
    )]
    pub authority_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Close<'info> {
    #[account(seeds = [b"admin".as_ref()], bump)]
    pub admin_settings: Account<'info, AdminSettings>,
    #[account(
        mut,
        seeds = [b"auction".as_ref(), auction.name.as_bytes().as_ref()],
        bump,
        close = authority
    )]
    pub auction: Account<'info, Auction>,
    #[account(
        mut,
        seeds = [b"proceeds".as_ref(), auction.key().as_ref()],
        bump
    )]
    pub proceeds: Account<'info, TokenAccount>,
    pub proceeds_mint: Account<'info, Mint>,
    /// CHECKED: custom constraint ensures the validity of this account
    #[account(
        mut, 
        constraint = auction.leader == leader.key()
    )]
    pub leader: UncheckedAccount<'info>,
    #[account(
        mut,
        associated_token::mint = proceeds_mint,
        associated_token::authority = authority
    )]
    pub authority_token_account: Account<'info, TokenAccount>,
    #[account(constraint = admin_settings.admin_key == authority.key())]
    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
}


#[account]
pub struct AdminSettings {
    pub admin_key: Pubkey,
}

#[account]
pub struct Auction {
    pub bump: u8,
    pub name: String,
    pub amount: u64,
    pub amount_increment: u64,
    pub lamports: u64,
    pub start_timestamp: i64,
    pub end_timestamp: i64,
    pub leader: Pubkey,
    pub leader_token_account: Pubkey,
}


#[error_code]
pub enum AuctionError {
    #[msg("Start timestamp must be smaller than end timestamp")]
    StartAfterEndTimestamp,
    #[msg("End timestamp already passed")]
    EndTimestampAlreadyPassed,
    #[msg("Auction has not yet started")]
    AuctionNotStarted,
    #[msg("Auction has ended")]
    AuctionEnded,
    #[msg("Auction name is too long")]
    NameTooLong,
    #[msg("Invalid calculation")]
    InvalidCalculation,
    #[msg("The new bid is not greater than the current best bid")]
    BidTooLow,
    #[msg("Auction has not yet finished")]
    AuctionNotFinished,
}
