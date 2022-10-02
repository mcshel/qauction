import assert from 'assert';
import * as spl from '@solana/spl-token';
import * as anchor from '@project-serum/anchor';
import { Program } from '@project-serum/anchor';
import { PublicKey } from '@solana/web3.js';
import { Qauction } from '../target/types/qauction';

describe('Healthy auction lifecycle', async () => {
    const program = anchor.workspace.Qauction as Program<Qauction>;

    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);
    
    const authoritySecret = JSON.parse(require('fs').readFileSync('/home/mpetac/.config/solana/id.json', 'utf8'));
    const authorityKeypair = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(authoritySecret));

    const name = Math.random().toString(36).slice(2, 7);//"test";
    const price = 10000;
    const price_increment = 1000;
    const auction_start = Math.floor(Date.now() / 1000);
    const auction_end = Math.floor(Date.now() / 1000 + 30);
    const programDataAccount = new PublicKey('ENzs3iXQgkqvfXVfz3CS6uhBr9FP7dsU1ZKiZneB29hf');
    const adminKeypair = anchor.web3.Keypair.generate();
    const falseAdminKeypair = anchor.web3.Keypair.generate();
    
    let proceedsMint: PublicKey;
    let adminSettingsAccount: PublicKey;
    let auctionAccount: PublicKey;
    let proceedsAccount: PublicKey;
    let adminTokenAccount: PublicKey;
    let falseAdminProceedsAccount: PublicKey;
    
    before( async () => {
        const airdropSignature1 = await provider.connection.requestAirdrop(adminKeypair.publicKey, 1e9);
        await provider.connection.confirmTransaction(airdropSignature1);
        
        const airdropSignature2 = await provider.connection.requestAirdrop(falseAdminKeypair.publicKey, 1e9);
        await provider.connection.confirmTransaction(airdropSignature2);
        
        
        //proceedsMint = spl.NATIVE_MINT;
        proceedsMint = await spl.createMint(provider.connection, authorityKeypair, authorityKeypair.publicKey, null, 6);
        
        let bump = null;
        [adminSettingsAccount, bump] = await anchor.web3.PublicKey.findProgramAddress([Buffer.from("admin")], program.programId);
        [auctionAccount, bump] = await anchor.web3.PublicKey.findProgramAddress([Buffer.from("auction"), Buffer.from(name)], program.programId);
        [proceedsAccount, bump] = await anchor.web3.PublicKey.findProgramAddress([Buffer.from("proceeds"), auctionAccount.toBuffer()], program.programId);
        
        console.log(`\t-------------------- Starting new auction --------------------`);
        console.log(`\tName             : ${name}`);
        console.log(`\tAuthority        : ${authorityKeypair.publicKey.toString()}`);
        console.log(`\tAdmin            : ${adminKeypair.publicKey.toString()}`);
        console.log(`\tMint             : ${proceedsMint.toString()}`);
        console.log(`\tAuction account  : ${auctionAccount.toString()}`);
        console.log(`\tProceeds account : ${proceedsAccount.toString()}`);
        console.log(`\t--------------------------------------------------------------`);
        
        if (proceedsMint == spl.NATIVE_MINT) {
            adminTokenAccount = await spl.createWrappedNativeAccount(provider.connection, adminKeypair, adminKeypair.publicKey, price);
            falseAdminProceedsAccount = await spl.createWrappedNativeAccount(provider.connection, falseAdminKeypair, falseAdminKeypair.publicKey, price);
        } else {
            adminTokenAccount = await spl.createAssociatedTokenAccount(provider.connection, adminKeypair, proceedsMint, adminKeypair.publicKey);
            const mintSignature1 = await spl.mintTo(provider.connection, authorityKeypair, proceedsMint, adminTokenAccount, authorityKeypair, price);
            console.log(`\tMint transaction: ${mintSignature1}`);
            
            falseAdminProceedsAccount = await spl.createAssociatedTokenAccount(provider.connection, falseAdminKeypair, proceedsMint, falseAdminKeypair.publicKey);
            const mintSignature2 = await spl.mintTo(provider.connection, authorityKeypair, proceedsMint, falseAdminProceedsAccount, authorityKeypair, price);
            console.log(`\tMint transaction: ${mintSignature2}`);
        }
        
    });
    
    it('Set raffles admin!', async () => {
        
        const adminSettingsInfo = await provider.connection.getAccountInfo(adminSettingsAccount);
        if (adminSettingsInfo) {
            const signature = await program.rpc.setAdmin(adminKeypair.publicKey, {
                accounts: {
                    adminSettings: adminSettingsAccount,
                    program: program.programId,
                    programData: programDataAccount,
                    authority: authorityKeypair.publicKey,
                },
            });
            console.log(`\tSet admin settings transaction: ${signature}`);
        } else {
            const signature = await program.rpc.initAdmin(adminKeypair.publicKey, {
                accounts: {
                    adminSettings: adminSettingsAccount,
                    program: program.programId,
                    programData: programDataAccount,
                    authority: authorityKeypair.publicKey,
                    systemProgram: anchor.web3.SystemProgram.programId,
                },
            });
            console.log(`\tInit admin settings transaction: ${signature}`);
        }
        
        const adminSettingsData = await program.account.adminSettings.fetch(adminSettingsAccount);
        assert.equal(adminSettingsData.adminKey.toString(), adminKeypair.publicKey.toString());
        
    });

    it('Initialized the auction!', async () => {
        
        const keypair = adminKeypair;
        const keypairTokenAccount = adminTokenAccount;
        //const keypair = falseAdminKeypair;
        //const keypairTokenAccount = falseAdminProceedsAccount;
        
        const tx = await program.transaction.initialize(name, new anchor.BN(price), new anchor.BN(price_increment), new anchor.BN(auction_start), new anchor.BN(auction_end), {
            accounts: {
                adminSettings: adminSettingsAccount,
                auction: auctionAccount,
                proceeds: proceedsAccount,
                proceedsMint: proceedsMint,
                authorityTokenAccount: keypairTokenAccount,
                authority: keypair.publicKey,
                tokenProgram: spl.TOKEN_PROGRAM_ID,
                systemProgram: anchor.web3.SystemProgram.programId,
                rent: anchor.web3.SYSVAR_RENT_PUBKEY,
            },
        });
        const signature = await anchor.web3.sendAndConfirmTransaction(provider.connection, tx, [keypair], {skipPreflight: true});
        console.log(`\tInitialize transaction: ${signature}`);
        
        const auctionAccountData = await program.account.auction.fetch(auctionAccount);
        assert.equal(auctionAccountData.name, name);
        assert.equal(auctionAccountData.amount, price);
        assert.equal(auctionAccountData.endTimestamp, auction_end);
    });
    
    it('Bid on the auction!', async () => {
        
        const amount = 2 * price;
        const bidderKeypair = anchor.web3.Keypair.generate();
        const airdropSignature = await provider.connection.requestAirdrop(bidderKeypair.publicKey, 1e9);
        await provider.connection.confirmTransaction(airdropSignature);
        
        let bidderTokenAccount;
        if (proceedsMint == spl.NATIVE_MINT) {
            bidderTokenAccount = await spl.createWrappedNativeAccount(provider.connection, bidderKeypair, bidderKeypair.publicKey, amount);
        } else {
            bidderTokenAccount = await spl.createAssociatedTokenAccount(provider.connection, bidderKeypair, proceedsMint, bidderKeypair.publicKey);
            const mintSignature = await spl.mintTo(provider.connection, authorityKeypair, proceedsMint, bidderTokenAccount, authorityKeypair, amount);
            console.log(`\tMint transaction: ${mintSignature}`);
        }
        
        let auctionAccountData = await program.account.auction.fetch(auctionAccount);
        const tx = await program.transaction.bid(new anchor.BN(amount), {
            accounts: {
                auction: auctionAccount,
                proceeds: proceedsAccount,
                proceedsMint: proceedsMint,
                leaderTokenAccount: auctionAccountData.leaderTokenAccount,
                leader: auctionAccountData.leader,
                authorityTokenAccount: bidderTokenAccount,
                authority: bidderKeypair.publicKey,
                tokenProgram: spl.TOKEN_PROGRAM_ID,
                systemProgram: anchor.web3.SystemProgram.programId,
            },
        });
        const signature = await anchor.web3.sendAndConfirmTransaction(provider.connection, tx, [bidderKeypair], {skipPreflight: true});
        console.log(`\tBid transaction: ${signature}`);
        
        auctionAccountData = await program.account.auction.fetch(auctionAccount);
        assert.equal(auctionAccountData.amount, amount);
        assert.equal(auctionAccountData.leader.toString(), bidderKeypair.publicKey.toString());
        assert.equal(auctionAccountData.leaderTokenAccount.toString(), bidderTokenAccount.toString());
    });
    
    it('Bid on the auction with creation of ATA!', async () => {
        
        const amount = 2 * price + price_increment;
        const bidderKeypair = anchor.web3.Keypair.generate();
        const airdropSignature = await provider.connection.requestAirdrop(bidderKeypair.publicKey, 1e9);
        await provider.connection.confirmTransaction(airdropSignature);
        console.log(`\tUser: ${bidderKeypair.publicKey}, amount: ${amount}`);
        
        let bidderTokenAccount;
        if (proceedsMint == spl.NATIVE_MINT) {
            bidderTokenAccount = await spl.createWrappedNativeAccount(provider.connection, bidderKeypair, bidderKeypair.publicKey, amount);
        } else {
            bidderTokenAccount = await spl.createAssociatedTokenAccount(provider.connection, bidderKeypair, proceedsMint, bidderKeypair.publicKey);
            const mintSignature = await spl.mintTo(provider.connection, authorityKeypair, proceedsMint, bidderTokenAccount, authorityKeypair, amount);
            console.log(`\tMint transaction: ${mintSignature}`);
        }
        
        
        let auctionAccountData = await program.account.auction.fetch(auctionAccount);
        const tx = await program.transaction.bid(new anchor.BN(amount), {
            accounts: {
                auction: auctionAccount,
                proceeds: proceedsAccount,
                proceedsMint: proceedsMint,
                leaderTokenAccount: auctionAccountData.leaderTokenAccount,
                leader: auctionAccountData.leader,
                authorityTokenAccount: bidderTokenAccount,
                authority: bidderKeypair.publicKey,
                tokenProgram: spl.TOKEN_PROGRAM_ID,
                //associatedTokenProgram: spl.ASSOCIATED_TOKEN_PROGRAM_ID,
                systemProgram: anchor.web3.SystemProgram.programId,
                //rent: anchor.web3.SYSVAR_RENT_PUBKEY,
            },
        });
        const signature = await anchor.web3.sendAndConfirmTransaction(provider.connection, tx, [bidderKeypair], {skipPreflight: true});
        console.log(`\tBid transaction: ${signature}`);
        
        
        
        await spl.closeAccount(provider.connection, bidderKeypair, bidderTokenAccount, bidderKeypair.publicKey, bidderKeypair);
        
        const amount2 = 2 * price + 2 * price_increment;
        const bidderKeypair2 = anchor.web3.Keypair.generate();
        const airdropSignature2 = await provider.connection.requestAirdrop(bidderKeypair2.publicKey, 1e9);
        await provider.connection.confirmTransaction(airdropSignature2);
        console.log(`\tUser: ${bidderKeypair2.publicKey}, amount: ${amount2}`);
        
        let bidderTokenAccount2;
        if (proceedsMint == spl.NATIVE_MINT) {
            bidderTokenAccount2 = await spl.createWrappedNativeAccount(provider.connection, bidderKeypair2, bidderKeypair2.publicKey, amount2);
        } else {
            bidderTokenAccount2 = await spl.createAssociatedTokenAccount(provider.connection, bidderKeypair2, proceedsMint, bidderKeypair2.publicKey);
            const mintSignature = await spl.mintTo(provider.connection, authorityKeypair, proceedsMint, bidderTokenAccount2, authorityKeypair, amount2);
            console.log(`\tMint transaction: ${mintSignature}`);
        }
        
        auctionAccountData = await program.account.auction.fetch(auctionAccount);
        const tx2 = await program.transaction.bidCreate(new anchor.BN(amount2), {
            accounts: {
                auction: auctionAccount,
                proceeds: proceedsAccount,
                proceedsMint: proceedsMint,
                leaderTokenAccount: auctionAccountData.leaderTokenAccount,
                leader: auctionAccountData.leader,
                authorityTokenAccount: bidderTokenAccount2,
                authority: bidderKeypair2.publicKey,
                tokenProgram: spl.TOKEN_PROGRAM_ID,
                associatedTokenProgram: spl.ASSOCIATED_TOKEN_PROGRAM_ID,
                systemProgram: anchor.web3.SystemProgram.programId,
                rent: anchor.web3.SYSVAR_RENT_PUBKEY,
            },
        });
        const signature2 = await anchor.web3.sendAndConfirmTransaction(provider.connection, tx2, [bidderKeypair2], {skipPreflight: true});
        console.log(`\tBid transaction: ${signature2}`);
    });
    
    it('Closed the auction!', async () => {
        
        await new Promise(f => setTimeout(f, 65000));
        
        console.log(`Auction end: ${auction_end}, current time: ${(Date.now() / 1000)}`);
        
        const keypair = adminKeypair;
        //const keypair = falseAdminKeypair;
        
        let adminTokenAccount = await spl.getOrCreateAssociatedTokenAccount(provider.connection, keypair, proceedsMint, keypair.publicKey);
        
        let auctionAccountData = await program.account.auction.fetch(auctionAccount);
        const tx = await program.transaction.close({
            accounts: {
                adminSettings: adminSettingsAccount,
                auction: auctionAccount,
                proceeds: proceedsAccount,
                proceedsMint: proceedsMint,
                leader: auctionAccountData.leader,
                authorityTokenAccount: adminTokenAccount.address,
                authority: keypair.publicKey,
                tokenProgram: spl.TOKEN_PROGRAM_ID,
            },
        });
        const signature = await anchor.web3.sendAndConfirmTransaction(provider.connection, tx, [keypair], {skipPreflight: true});
        console.log(`\tClose transaction: ${signature}`);
    });
    
    
});
