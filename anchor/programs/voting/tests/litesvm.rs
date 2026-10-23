use anchor_lang::declare_program;
use anchor_litesvm::{AnchorContext, AnchorLiteSVM, AssertionHelpers, Pubkey, Signer, TestHelpers};
use anchor_lang::prelude::*;

declare_program!(voting);

use self::voting::accounts::{CandidateAccount, PollAccount};
use self::voting::client::{accounts, args};

const PROGRAM_BYTES: &[u8] = include_bytes!("../../../target/deploy/voting.so");

fn setup() -> AnchorContext {
    use anchor_lang::solana_program::clock::Clock;

    let mut ctx = AnchorLiteSVM::build_with_program(self::voting::ID, PROGRAM_BYTES);

    let clock = Clock {
        slot: 1000,
        epoch_start_timestamp: 0,
        epoch: 1,
        leader_schedule_epoch: 1,
        unix_timestamp: 1000,
    };

    ctx.svm.set_sysvar(&clock);
    ctx
}

fn get_poll_pda(poll_id: u64) -> Pubkey {
    Pubkey::find_program_address(&[b"poll", &poll_id.to_le_bytes()], &self::voting::ID).0
}

fn get_candidate_pda(poll_id: u64, candidate: &str) -> Pubkey {
    Pubkey::find_program_address(&[&poll_id.to_le_bytes(), candidate.as_bytes()], &self::voting::ID).0
}

fn get_receipt_pda(poll_id: u64, voter: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"receipt", &poll_id.to_le_bytes(), voter.as_ref()],
        &self::voting::ID,
    )
    .0
}

fn try_initialize_poll(
    ctx: &mut AnchorContext,
    signer: &anchor_litesvm::Keypair,
    poll_id: u64,
    start_time: u64,
    end_time: u64,
    name: &str,
    description: &str,
) -> anchor_litesvm::TransactionResult {
    let poll_pda = get_poll_pda(poll_id);

    let ix = ctx
        .program()
        .accounts(accounts::InitializePoll {
            signer: signer.pubkey(),
            poll_account: poll_pda,
            system_program: anchor_lang::system_program::ID,
        })
        .args(args::InitializePoll {
            _poll_id: poll_id,
            start_time,
            end_time,
            name: name.to_string(),
            description: description.to_string(),
        })
        .instruction()
        .unwrap();

    ctx.execute_instruction(ix, &[signer]).unwrap()
}

fn initialize_poll(
    ctx: &mut AnchorContext,
    signer: &anchor_litesvm::Keypair,
    poll_id: u64,
    start_time: u64,
    end_time: u64,
    name: &str,
    description: &str,
) {
    try_initialize_poll(ctx, signer, poll_id, start_time, end_time, name, description)
        .assert_success();
    ctx.svm.assert_account_exists(&get_poll_pda(poll_id));
}

fn initialize_candidate(
    ctx: &mut AnchorContext,
    signer: &anchor_litesvm::Keypair,
    poll_id: u64,
    candidate: &str,
) {
    let ix = ctx
        .program()
        .accounts(accounts::InitializeCandidate {
            signer: signer.pubkey(),
            poll_account: get_poll_pda(poll_id),
            candidate_account: get_candidate_pda(poll_id, candidate),
            system_program: anchor_lang::system_program::ID,
        })
        .args(args::InitializeCandidate {
            _poll_id: poll_id,
            candidate: candidate.to_string(),
        })
        .instruction()
        .unwrap();

    let result = ctx.execute_instruction(ix, &[signer]).unwrap();
    result.assert_success();
}

fn cast_vote(
    ctx: &mut AnchorContext,
    signer: &anchor_litesvm::Keypair,
    poll_id: u64,
    candidate: &str,
) -> anchor_litesvm::TransactionResult {
    let ix = ctx
        .program()
        .accounts(accounts::Vote {
            signer: signer.pubkey(),
            poll_account: get_poll_pda(poll_id),
            candidate_account: get_candidate_pda(poll_id, candidate),
            vote_receipt: get_receipt_pda(poll_id, &signer.pubkey()),
            system_program: anchor_lang::system_program::ID,
        })
        .args(args::Vote {
            _poll_id: poll_id,
            _candidate: candidate.to_string(),
        })
        .instruction()
        .unwrap();

    ctx.execute_instruction(ix, &[signer]).unwrap()
}

#[test]
fn test_init_poll() {
    let mut ctx = setup();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let poll_id = 1;
    let poll_pda = get_poll_pda(poll_id);
    let start_time = 0;
    let end_time = u64::MAX;
    let poll_name = "Test Poll";
    let poll_description = "A test poll for voting";

    initialize_poll(
        &mut ctx,
        &user,
        poll_id,
        start_time,
        end_time,
        poll_name,
        poll_description,
    );

    let poll_account: PollAccount = ctx.get_account(&poll_pda).unwrap();
    assert_eq!(poll_account.poll_name, poll_name);
    assert_eq!(poll_account.poll_description, poll_description);
    assert_eq!(poll_account.poll_voting_start, start_time);
    assert_eq!(poll_account.poll_voting_end, end_time);
    assert_eq!(poll_account.poll_option_index, 0);
}

#[test]
fn test_init_candidates() {
    let mut ctx = setup();
    let user = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let poll_id = 1;
    initialize_poll(
        &mut ctx,
        &user,
        poll_id,
        0,
        u64::MAX,
        "Test Poll",
        "A test poll for voting",
    );
    initialize_candidate(&mut ctx, &user, poll_id, "Alice");
    initialize_candidate(&mut ctx, &user, poll_id, "Bob");

    let poll_account: PollAccount = ctx.get_account(&get_poll_pda(poll_id)).unwrap();
    let alice_account: CandidateAccount = ctx.get_account(&get_candidate_pda(poll_id, "Alice")).unwrap();
    let bob_account: CandidateAccount = ctx.get_account(&get_candidate_pda(poll_id, "Bob")).unwrap();

    assert_eq!(poll_account.poll_option_index, 2);
    assert_eq!(alice_account.candidate_name, "Alice");
    assert_eq!(alice_account.candidate_votes, 0);
    assert_eq!(bob_account.candidate_name, "Bob");
    assert_eq!(bob_account.candidate_votes, 0);
}

#[test]
fn test_vote() {
    let mut ctx = setup();
    let authority = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let voter = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let poll_id = 1;
    initialize_poll(
        &mut ctx,
        &authority,
        poll_id,
        0,
        2_000,
        "Test Poll",
        "A test poll for voting",
    );
    initialize_candidate(&mut ctx, &authority, poll_id, "Alice");

    let result = cast_vote(&mut ctx, &voter, poll_id, "Alice");
    result.assert_success();

    let alice_account: CandidateAccount = ctx.get_account(&get_candidate_pda(poll_id, "Alice")).unwrap();
    assert_eq!(alice_account.candidate_votes, 1);
}

#[test]
fn test_vote_before_start_fails() {
    let mut ctx = setup();
    let authority = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let voter = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let poll_id = 1;
    initialize_poll(
        &mut ctx,
        &authority,
        poll_id,
        1_500,
        2_000,
        "Future Poll",
        "Voting has not started yet",
    );
    initialize_candidate(&mut ctx, &authority, poll_id, "Alice");

    cast_vote(&mut ctx, &voter, poll_id, "Alice")
        .assert_failure()
        .assert_anchor_error("VotingNotStarted");
}

#[test]
fn test_vote_after_end_fails() {
    let mut ctx = setup();
    let authority = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let voter = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let poll_id = 1;
    initialize_poll(
        &mut ctx,
        &authority,
        poll_id,
        0,
        500,
        "Closed Poll",
        "Voting has ended",
    );
    initialize_candidate(&mut ctx, &authority, poll_id, "Alice");

    cast_vote(&mut ctx, &voter, poll_id, "Alice")
        .assert_failure()
        .assert_anchor_error("VotingEnded");
}

#[test]
fn test_double_vote_fails() {
    let mut ctx = setup();
    let authority = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let voter = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let poll_id = 1;
    initialize_poll(
        &mut ctx,
        &authority,
        poll_id,
        0,
        2_000,
        "Test Poll",
        "A test poll for voting",
    );
    initialize_candidate(&mut ctx, &authority, poll_id, "Alice");
    initialize_candidate(&mut ctx, &authority, poll_id, "Bob");

    cast_vote(&mut ctx, &voter, poll_id, "Alice").assert_success();

    // Retrying the same vote. The blockhash has to change, otherwise the
    // runtime rejects the resend as a duplicate and the program never runs.
    ctx.svm.expire_blockhash();
    cast_vote(&mut ctx, &voter, poll_id, "Alice").assert_failure();

    // Switching candidates is a distinct transaction, so this one really does
    // reach the program and gets turned away by the receipt PDA.
    cast_vote(&mut ctx, &voter, poll_id, "Bob").assert_failure();

    let alice_account: CandidateAccount = ctx.get_account(&get_candidate_pda(poll_id, "Alice")).unwrap();
    let bob_account: CandidateAccount = ctx.get_account(&get_candidate_pda(poll_id, "Bob")).unwrap();
    assert_eq!(alice_account.candidate_votes, 1);
    assert_eq!(bob_account.candidate_votes, 0);
}

#[test]
fn test_second_voter_can_still_vote() {
    let mut ctx = setup();
    let authority = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let first = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let second = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let poll_id = 1;
    initialize_poll(
        &mut ctx,
        &authority,
        poll_id,
        0,
        2_000,
        "Test Poll",
        "A test poll for voting",
    );
    initialize_candidate(&mut ctx, &authority, poll_id, "Alice");

    cast_vote(&mut ctx, &first, poll_id, "Alice").assert_success();
    cast_vote(&mut ctx, &second, poll_id, "Alice").assert_success();

    let alice_account: CandidateAccount = ctx.get_account(&get_candidate_pda(poll_id, "Alice")).unwrap();
    assert_eq!(alice_account.candidate_votes, 2);
}

#[test]
fn test_reinitialize_poll_fails() {
    let mut ctx = setup();
    let authority = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let attacker = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let poll_id = 1;
    initialize_poll(
        &mut ctx,
        &authority,
        poll_id,
        0,
        2_000,
        "Test Poll",
        "A test poll for voting",
    );

    // Re-running initialize_poll on a live poll must not rewrite its window.
    try_initialize_poll(
        &mut ctx,
        &attacker,
        poll_id,
        9_000,
        9_999,
        "Hijacked Poll",
        "Window moved out from under the voters",
    )
    .assert_failure();

    let poll_account: PollAccount = ctx.get_account(&get_poll_pda(poll_id)).unwrap();
    assert_eq!(poll_account.poll_name, "Test Poll");
    assert_eq!(poll_account.poll_voting_start, 0);
    assert_eq!(poll_account.poll_voting_end, 2_000);
}
