import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { GfxTask } from "../target/types/gfx_task";
import {
  getAccount,
  getAssociatedTokenAddressSync,
  getOrCreateAssociatedTokenAccount,
} from "@solana/spl-token";
import { Connection } from "@solana/web3.js";
import {
  createMint,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  mintTo,
} from "@solana/spl-token";
import { expect } from "chai";

// Pubkey: 4jURAvf4NbrLki15eNWuojugzENbAntJdd7NP5FFgy3q
const ADMIN_SECRET_KEY = new Uint8Array([
  69, 68, 185, 100, 166, 123, 45, 126, 37, 165, 98, 38, 150, 101, 126, 107, 45,
  235, 16, 162, 194, 6, 75, 5, 62, 193, 14, 161, 85, 153, 182, 146, 55, 116,
  116, 120, 249, 39, 8, 116, 187, 0, 66, 72, 2, 229, 216, 245, 40, 82, 10, 117,
  223, 83, 175, 93, 248, 157, 139, 177, 192, 58, 19, 60,
]);
const CONNECTION = new Connection("http://localhost:8899", "confirmed");
const TEST_SPL_TOKEN_DECIMALS = 9;

const airdrop = async (pubkey) => {
  const airdropSignature = await CONNECTION.requestAirdrop(
    pubkey,
    2 * anchor.web3.LAMPORTS_PER_SOL
  );
  await CONNECTION.confirmTransaction(airdropSignature);
};

describe("gfx-task", () => {
  let mint,
    userATA,
    programAdminATA,
    payInterestSigner,
    userKeypair,
    programAdmin;

  before(async () => {
    const provider = new anchor.AnchorProvider(
      CONNECTION,
      anchor.Wallet.local(),
      {}
    );
    anchor.setProvider(provider);

    userKeypair = anchor.web3.Keypair.generate();
    programAdmin = anchor.web3.Keypair.fromSecretKey(ADMIN_SECRET_KEY);
    payInterestSigner = anchor.web3.Keypair.generate();
    const mintAuthority = anchor.web3.Keypair.generate();

    // airdrop SOL into wallets we use to pay for txs
    await Promise.all(
      [
        userKeypair.publicKey,
        programAdmin.publicKey,
        mintAuthority.publicKey,
        payInterestSigner.publicKey,
      ].map(airdrop)
    );

    // create a new SPL token mint
    mint = await createMint(
      CONNECTION,
      mintAuthority,
      mintAuthority.publicKey,
      null,
      TEST_SPL_TOKEN_DECIMALS
    );

    // Create an associated token account for the payer
    userATA = await getOrCreateAssociatedTokenAccount(
      CONNECTION,
      userKeypair,
      mint,
      userKeypair.publicKey
    );

    // Mint tokens in the user keypair
    await mintTo(
      CONNECTION,
      userKeypair,
      mint,
      userATA.address,
      mintAuthority,
      100 * 10 ** TEST_SPL_TOKEN_DECIMALS
    );

    // Create an associated token account for the program authority to send to the treasury
    programAdminATA = await getOrCreateAssociatedTokenAccount(
      CONNECTION,
      userKeypair,
      mint,
      programAdmin.publicKey
    );

    // Mint tokens in the user keypair
    await mintTo(
      CONNECTION,
      userKeypair,
      mint,
      programAdminATA.address,
      mintAuthority,
      10000 * 10 ** TEST_SPL_TOKEN_DECIMALS
    );
  });

  it("works", async () => {
    const program = anchor.workspace.GfxTask as Program<GfxTask>;

    /**************************
     *  Init Treasury Ix
     * ************************/
    const [treasuryAuthorityPDA, tabump] =
      anchor.web3.PublicKey.findProgramAddressSync(
        [anchor.utils.bytes.utf8.encode("gfx_task_treasury"), mint.toBuffer()],
        program.programId
      );

    const treasuryTokenAccount = getAssociatedTokenAddressSync(
      mint,
      treasuryAuthorityPDA,
      true
    );
    const amountInitTreasury = 10 * 10 ** TEST_SPL_TOKEN_DECIMALS;

    await program.methods
      .initializeTreasury(new anchor.BN(amountInitTreasury))
      .accounts({
        mint: mint,
        treasuryAuthority: treasuryAuthorityPDA,
        treasuryTokenAccount: treasuryTokenAccount,
        adminAta: programAdminATA.address,
        admin: programAdmin.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([programAdmin])
      .rpc({ skipPreflight: true });

    const treasuryTokenAccountOnChain = await getAccount(
      CONNECTION,
      treasuryTokenAccount
    );
    expect(BigInt(amountInitTreasury)).to.equal(
      treasuryTokenAccountOnChain.amount
    );

    /**************************
     *  Init Vault Ix
     * ************************/
    const [vaultPDA, _vpbump] = anchor.web3.PublicKey.findProgramAddressSync(
      [
        anchor.utils.bytes.utf8.encode("gfx_task_vault"),
        userKeypair.publicKey.toBuffer(),
      ],
      program.programId
    );

    const vaultTokenAccount = getAssociatedTokenAddressSync(
      mint,
      vaultPDA,
      true
    );

    await program.methods
      .initializeVault()
      .accounts({
        mint,
        vault: vaultPDA,
        vaultTokenAccount: vaultTokenAccount,
        user: userKeypair.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([userKeypair])
      .rpc({ skipPreflight: true });

    /**************************
     *  Deposit Ix
     * ************************/
    const amountDeposit = 10 * 10 ** TEST_SPL_TOKEN_DECIMALS;

    await program.methods
      .deposit(new anchor.BN(amountDeposit))
      .accounts({
        mint,
        vault: vaultPDA,
        vaultTokenAccount,
        fromAta: userATA.address,
        from: userKeypair.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([userKeypair])
      .rpc({ skipPreflight: true });

    const vaultTokenAccountOnChain = await getAccount(
      CONNECTION,
      vaultTokenAccount
    );
    expect(BigInt(amountDeposit)).to.equal(vaultTokenAccountOnChain.amount);

    /**************************
     *  Pay Interest Ix
     * ************************/

    if (process.env.INCLUDE_PAY_INTEREST) {
      let vaultAmountPre = (await getAccount(CONNECTION, vaultTokenAccount))
        .amount;

      await program.methods
        .payInterest(tabump)
        .accounts({
          mint,
          user: userKeypair.publicKey,
          vault: vaultPDA,
          vaultTokenAccount,
          treasuryAuthority: treasuryAuthorityPDA,
          treasuryTokenAccount: treasuryTokenAccount,
          signer: payInterestSigner.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([payInterestSigner])
        .rpc({ skipPreflight: true });

      const vaultAmountPost = (await getAccount(CONNECTION, vaultTokenAccount))
        .amount;
      expect(vaultAmountPre + vaultAmountPre / BigInt(100)).to.equal(
        vaultAmountPost
      );

      let payInterestError;
      try {
        await program.methods
          .payInterest(tabump)
          .accounts({
            mint,
            user: userKeypair.publicKey,
            vault: vaultPDA,
            vaultTokenAccount,
            treasuryAuthority: treasuryAuthorityPDA,
            treasuryTokenAccount: treasuryTokenAccount,
            signer: payInterestSigner.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            systemProgram: anchor.web3.SystemProgram.programId,
          })
          .signers([payInterestSigner])
          .rpc({ skipPreflight: true });
      } catch (e) {
        payInterestError = e;
      } finally {
        const errorCode = payInterestError?.error?.errorCode?.code;
        expect(errorCode).to.equal("InterestAlreadyPaid");
      }
    }
  });
});
