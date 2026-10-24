extern crate std;

use super::*;
use soroban_sdk::testutils::Address as _;

/// Contract functions panic with typed errors, so `try_` calls surface them as host errors.
fn err(e: Error) -> soroban_sdk::Error {
    soroban_sdk::Error::from_contract_error(e as u32)
}

struct Setup<'a> {
    env: Env,
    admin: Address,
    client: PropertyTokenClient<'a>,
}

fn setup() -> Setup<'static> {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let id = env.register(
        PropertyToken,
        (
            admin.clone(),
            String::from_str(&env, "prop-001"),
            String::from_str(&env, "Lekki Garden Shares"),
            String::from_str(&env, "LKGS"),
            1_000i128,
        ),
    );
    let client = PropertyTokenClient::new(&env, &id);
    Setup { env, admin, client }
}

fn investor(s: &Setup) -> Address {
    let a = Address::generate(&s.env);
    s.client.set_whitelisted(&a, &true);
    a
}

#[test]
fn constructor_sets_metadata() {
    let s = setup();
    assert_eq!(s.client.admin(), s.admin);
    assert_eq!(s.client.symbol(), String::from_str(&s.env, "LKGS"));
    assert_eq!(s.client.max_supply(), 1_000);
    assert_eq!(s.client.total_supply(), 0);
    assert_eq!(s.client.decimals(), 0);
}

#[test]
fn mint_and_transfer_between_whitelisted_accounts() {
    let s = setup();
    let alice = investor(&s);
    let bob = investor(&s);

    s.client.mint(&alice, &600);
    s.client.transfer(&alice, &bob, &150);

    assert_eq!(s.client.balance(&alice), 450);
    assert_eq!(s.client.balance(&bob), 150);
    assert_eq!(s.client.total_supply(), 600);
}

#[test]
fn mint_requires_whitelist() {
    let s = setup();
    let stranger = Address::generate(&s.env);
    assert_eq!(
        s.client.try_mint(&stranger, &1),
        Err(Ok(err(Error::NotWhitelisted)))
    );
}

#[test]
fn mint_cannot_exceed_max_supply() {
    let s = setup();
    let alice = investor(&s);
    s.client.mint(&alice, &1_000);
    assert_eq!(
        s.client.try_mint(&alice, &1),
        Err(Ok(err(Error::MaxSupplyExceeded)))
    );
}

#[test]
fn transfer_to_non_whitelisted_fails() {
    let s = setup();
    let alice = investor(&s);
    let stranger = Address::generate(&s.env);
    s.client.mint(&alice, &10);
    assert_eq!(
        s.client.try_transfer(&alice, &stranger, &5),
        Err(Ok(err(Error::NotWhitelisted)))
    );
}

#[test]
fn transfer_more_than_balance_fails() {
    let s = setup();
    let alice = investor(&s);
    let bob = investor(&s);
    s.client.mint(&alice, &10);
    assert_eq!(
        s.client.try_transfer(&alice, &bob, &11),
        Err(Ok(err(Error::InsufficientBalance)))
    );
}

#[test]
fn rejects_non_positive_amounts() {
    let s = setup();
    let alice = investor(&s);
    assert_eq!(
        s.client.try_mint(&alice, &0),
        Err(Ok(err(Error::InvalidAmount)))
    );
}

#[test]
fn removing_from_whitelist_blocks_incoming_transfers() {
    let s = setup();
    let alice = investor(&s);
    let bob = investor(&s);
    s.client.mint(&alice, &10);
    s.client.set_whitelisted(&bob, &false);
    assert!(!s.client.is_whitelisted(&bob));
    assert_eq!(
        s.client.try_transfer(&alice, &bob, &1),
        Err(Ok(err(Error::NotWhitelisted)))
    );
}

#[test]
#[should_panic]
fn mint_requires_admin_auth() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let id = env.register(
        PropertyToken,
        (
            admin,
            String::from_str(&env, "prop-001"),
            String::from_str(&env, "Shares"),
            String::from_str(&env, "SHR"),
            100i128,
        ),
    );
    let client = PropertyTokenClient::new(&env, &id);
    // No mocked auths: the admin has not signed, so this must panic.
    client.mint(&Address::generate(&env), &1);
}

#[test]
fn burn_reduces_balance_and_supply() {
    let s = setup();
    let alice = investor(&s);
    s.client.mint(&alice, &100);
    s.client.burn(&alice, &40);
    assert_eq!(s.client.balance(&alice), 60);
    assert_eq!(s.client.total_supply(), 60);
}

#[test]
fn burn_more_than_balance_fails() {
    let s = setup();
    let alice = investor(&s);
    s.client.mint(&alice, &10);
    assert_eq!(
        s.client.try_burn(&alice, &11),
        Err(Ok(err(Error::InsufficientBalance)))
    );
}

#[test]
fn burn_rejects_zero_amount() {
    let s = setup();
    let alice = investor(&s);
    s.client.mint(&alice, &10);
    assert_eq!(
        s.client.try_burn(&alice, &0),
        Err(Ok(err(Error::InvalidAmount)))
    );
}

#[test]
#[should_panic]
fn burn_requires_admin_auth() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let id = env.register(
        PropertyToken,
        (
            admin,
            String::from_str(&env, "prop-001"),
            String::from_str(&env, "Shares"),
            String::from_str(&env, "SHR"),
            100i128,
        ),
    );
    let client = PropertyTokenClient::new(&env, &id);
    // No mocked auths: the admin has not signed, so this must panic.
    client.burn(&Address::generate(&env), &1);
}

#[test]
fn version_returns_crate_version() {
    let s = setup();
    assert_eq!(s.client.version(), VERSION);
}

#[test]
fn admin_can_upgrade_contract() {
    let s = setup();
    let new_wasm_hash = s.env.deployer().upload_contract_wasm(PropertyTokenWasm);
    s.client.upgrade(&new_wasm_hash);
    // Contract remains callable after the in-place upgrade.
    assert_eq!(s.client.version(), VERSION);
}

#[test]
fn non_admin_upgrade_fails() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let id = env.register(
        PropertyToken,
        (
            admin,
            String::from_str(&env, "prop-001"),
            String::from_str(&env, "Shares"),
            String::from_str(&env, "SHR"),
            100i128,
        ),
    );
    let client = PropertyTokenClient::new(&env, &id);
    let new_wasm_hash = env.deployer().upload_contract_wasm(PropertyTokenWasm);
    // No mocked auths: the admin has not signed, so this must panic.
    client.upgrade(&new_wasm_hash);
}

#[test]
fn total_supply_equals_sum_of_balances_after_mint_transfer_burn() {
    let s = setup();
    let alice = investor(&s);
    let bob = investor(&s);

    s.client.mint(&alice, &500);
    s.client.mint(&bob, &300);
    s.client.transfer(&alice, &bob, &200);
    s.client.burn(&bob, &100);

    let sum = s.client.balance(&alice) + s.client.balance(&bob);
    assert_eq!(s.client.total_supply(), sum);
    assert_eq!(s.client.total_supply(), 700);
}

#[test]
fn approve_and_spend_part_of_allowance() {
    let s = setup();
    let alice = investor(&s);
    let bob = investor(&s);
    s.client.mint(&alice, &100);
    let expiration = s.env.ledger().sequence() + 100;
    s.client.approve(&alice, &bob, &50, &expiration);
    assert_eq!(s.client.allowance(&alice, &bob), 50);

    s.client.transfer_from(&bob, &alice, &bob, &20);
    assert_eq!(s.client.balance(&alice), 80);
    assert_eq!(s.client.balance(&bob), 20);
    assert_eq!(s.client.allowance(&alice, &bob), 30);
}

#[test]
fn transfer_from_more_than_allowance_fails() {
    let s = setup();
    let alice = investor(&s);
    let bob = investor(&s);
    s.client.mint(&alice, &100);
    let expiration = s.env.ledger().sequence() + 100;
    s.client.approve(&alice, &bob, &10, &expiration);
    assert_eq!(
        s.client.try_transfer_from(&bob, &alice, &bob, &11),
        Err(Ok(err(Error::InsufficientAllowance)))
    );
}

#[test]
fn expired_allowance_fails() {
    let s = setup();
    let alice = investor(&s);
    let bob = investor(&s);
    s.client.mint(&alice, &100);
    let expiration = s.env.ledger().sequence() + 1;
    s.client.approve(&alice, &bob, &50, &expiration);
    s.env.ledger().set_sequence_number(expiration + 1);
    assert_eq!(
        s.client.try_transfer_from(&bob, &alice, &bob, &1),
        Err(Ok(err(Error::InsufficientAllowance)))
    );
}

#[test]
fn transfer_from_to_non_whitelisted_fails() {
    let s = setup();
    let alice = investor(&s);
    let bob = investor(&s);
    let stranger = Address::generate(&s.env);
    s.client.mint(&alice, &100);
    let expiration = s.env.ledger().sequence() + 100;
    s.client.approve(&alice, &bob, &50, &expiration);
    assert_eq!(
        s.client.try_transfer_from(&bob, &alice, &stranger, &1),
        Err(Ok(err(Error::NotWhitelisted)))
    );
}

#[test]
fn readable_through_sep41_token_client() {
    let s = setup();
    let alice = investor(&s);
    s.client.mint(&alice, &42);
    let token = soroban_sdk::token::TokenClient::new(&s.env, &s.client.address);
    assert_eq!(token.balance(&alice), 42);
    assert_eq!(token.decimals(), 0);
    assert_eq!(token.symbol(), String::from_str(&s.env, "LKGS"));
}

#[test]
fn paused_blocks_mint_and_transfer() {
    let s = setup();
    let alice = investor(&s);
    let bob = investor(&s);
    s.client.mint(&alice, &10);
    s.client.pause();
    assert!(s.client.is_paused());
    assert_eq!(
        s.client.try_mint(&alice, &1),
        Err(Ok(err(Error::Paused)))
    );
    assert_eq!(
        s.client.try_transfer(&alice, &bob, &1),
        Err(Ok(err(Error::Paused)))
    );
    // Views and whitelist management keep working while paused.
    assert_eq!(s.client.balance(&alice), 10);
    s.client.set_whitelisted(&bob, &true);
}

#[test]
fn unpause_restores_mint_and_transfer() {
    let s = setup();
    let alice = investor(&s);
    let bob = investor(&s);
    s.client.mint(&alice, &10);
    s.client.pause();
    s.client.unpause();
    assert!(!s.client.is_paused());
    s.client.mint(&alice, &5);
    s.client.transfer(&alice, &bob, &3);
    assert_eq!(s.client.balance(&alice), 12);
    assert_eq!(s.client.balance(&bob), 3);
}

#[test]
#[should_panic]
fn pause_requires_admin_auth() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let id = env.register(
        PropertyToken,
        (
            admin,
            String::from_str(&env, "prop-001"),
            String::from_str(&env, "Shares"),
            String::from_str(&env, "SHR"),
            100i128,
        ),
    );
    let client = PropertyTokenClient::new(&env, &id);
    // No mocked auths: the admin has not signed, so this must panic.
    client.pause();
}

#[test]
#[should_panic]
fn unpause_requires_admin_auth() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let id = env.register(
        PropertyToken,
        (
            admin,
            String::from_str(&env, "prop-001"),
            String::from_str(&env, "Shares"),
            String::from_str(&env, "SHR"),
            100i128),
    );
    let client = PropertyTokenClient::new(&env, &id);
    // No mocked auths: the admin has not signed, so this must panic.
    client.unpause();
}

#[test]
fn propose_and_accept_admin() {
    let s = setup();
    let new_admin = Address::generate(&s.env);
    s.client.propose_admin(&new_admin);
    assert_eq!(s.client.pending_admin(), Some(new_admin.clone()));
    // Admin is unchanged until the new admin accepts.
    assert_eq!(s.client.admin(), s.admin);
    s.client.accept_admin();
    assert_eq!(s.client.admin(), new_admin);
    assert_eq!(s.client.pending_admin(), None);
}

#[test]
fn accept_admin_by_wrong_account_fails() {
    let s = setup();
    let new_admin = Address::generate(&s.env);
    s.client.propose_admin(&new_admin);
    // Only the pending admin may accept; a different account must not.
    let impostor = Address::generate(&s.env);
    s.env.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: &impostor,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &s.client.address,
            fn_name: "accept_admin",
            args: ().into_val(&s.env),
            sub_invokes: &[],
        },
    }]);
    assert!(s.client.try_accept_admin().is_err());
    assert_eq!(s.client.admin(), s.admin);
}

#[test]
fn cancel_pending_admin() {
    let s = setup();
    let new_admin = Address::generate(&s.env);
    s.client.propose_admin(&new_admin);
    s.client.cancel_admin_proposal();
    assert_eq!(s.client.pending_admin(), None);
    assert_eq!(s.client.admin(), s.admin);
}

#[test]
fn bump_keeps_balances_live_past_original_ttl() {
    let s = setup();
    let alice = investor(&s);
    let bob = investor(&s);
    s.client.mint(&alice, &600);
    s.client.mint(&bob, &400);

    // Advance well past the original TTL so the entries would otherwise be archived.
    s.env
        .ledger()
        .set_sequence_number(s.env.ledger().sequence() + TTL_EXTEND_TO + 1);

    s.client.bump(&Vec::from_array(&s.env, [alice.clone(), bob.clone()]));

    // Entries are still live and readable after the bump.
    assert_eq!(s.client.balance(&alice), 600);
    assert_eq!(s.client.balance(&bob), 400);
    assert_eq!(s.client.total_supply(), 1_000);
}

#[test]
fn bump_with_account_without_balance_entry_does_not_panic() {
    let s = setup();
    let alice = investor(&s);
    let stranger = Address::generate(&s.env);
    s.client.mint(&alice, &10);

    // Bumping an account that has no balance entry must not panic.
    s.client
        .bump(&Vec::from_array(&s.env, [alice.clone(), stranger]));

    assert_eq!(s.client.balance(&alice), 10);
}

#[test]
fn max_per_holder_defaults_to_none() {
    let s = setup();
    assert_eq!(s.client.max_per_holder(), None);
}

#[test]
fn set_max_per_holder_updates_view() {
    let s = setup();
    s.client.set_max_per_holder(&Some(100));
    assert_eq!(s.client.max_per_holder(), Some(100));
    s.client.set_max_per_holder(&None);
    assert_eq!(s.client.max_per_holder(), None);
}

#[test]
fn mint_at_holder_limit_succeeds() {
    let s = setup();
    let alice = investor(&s);
    s.client.set_max_per_holder(&Some(100));
    s.client.mint(&alice, &100);
    assert_eq!(s.client.balance(&alice), 100);
}

#[test]
fn mint_over_holder_limit_fails() {
    let s = setup();
    let alice = investor(&s);
    s.client.set_max_per_holder(&Some(100));
    s.client.mint(&alice, &100);
    assert_eq!(
        s.client.try_mint(&alice, &1),
        Err(Ok(err(Error::HolderLimitExceeded)))
    );
    assert_eq!(s.client.balance(&alice), 100);
}

#[test]
fn transfer_at_holder_limit_succeeds() {
    let s = setup();
    let alice = investor(&s);
    let bob = investor(&s);
    s.client.set_max_per_holder(&Some(100));
    s.client.mint(&alice, &200);
    s.client.transfer(&alice, &bob, &100);
    assert_eq!(s.client.balance(&bob), 100);
}

#[test]
fn transfer_over_holder_limit_fails() {
    let s = setup();
    let alice = investor(&s);
    let bob = investor(&s);
    s.client.set_max_per_holder(&Some(100));
    s.client.mint(&alice, &200);
    s.client.mint(&bob, &50);
    assert_eq!(
        s.client.try_transfer(&alice, &bob, &51),
        Err(Ok(err(Error::HolderLimitExceeded)))
    );
    assert_eq!(s.client.balance(&bob), 50);
}

#[test]
fn no_holder_limit_allows_any_amount() {
    let s = setup();
    let alice = investor(&s);
    let bob = investor(&s);
    s.client.mint(&alice, &1_000);
    s.client.transfer(&alice, &bob, &1_000);
    assert_eq!(s.client.balance(&bob), 1_000);
}

#[test]
fn frozen_account_cannot_send_or_receive() {
    let s = setup();
    let alice = investor(&s);
    let bob = investor(&s);
    let carol = investor(&s);
    s.client.mint(&alice, &100);
    s.client.mint(&bob, &100);

    s.client.freeze(&bob);
    assert!(s.client.is_frozen(&bob));

    // Frozen account cannot send.
    assert_eq!(
        s.client.try_transfer(&bob, &carol, &1),
        Err(Ok(err(Error::AccountFrozen)))
    );
    // Frozen account cannot receive.
    assert_eq!(
        s.client.try_transfer(&alice, &bob, &1),
        Err(Ok(err(Error::AccountFrozen)))
    );

    // Other holders still transact freely.
    s.client.transfer(&alice, &carol, &10);
    assert_eq!(s.client.balance(&carol), 10);
}

#[test]
fn unfreeze_restores_transfers() {
    let s = setup();
    let alice = investor(&s);
    let bob = investor(&s);
    s.client.mint(&alice, &100);
    s.client.freeze(&bob);
    s.client.unfreeze(&bob);
    assert!(!s.client.is_frozen(&bob));
    s.client.transfer(&alice, &bob, &10);
    assert_eq!(s.client.balance(&bob), 10);
}

#[test]
#[should_panic]
fn freeze_requires_admin_auth() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let id = env.register(
        PropertyToken,
        (
            admin,
            String::from_str(&env, "prop-001"),
            String::from_str(&env, "Shares"),
            String::from_str(&env, "SHR"),
            100i128,
        ),
    );
    let client = PropertyTokenClient::new(&env, &id);
    // No mocked auths: the admin has not signed, so this must panic.
    client.freeze(&Address::generate(&env));
}

#[test]
#[should_panic]
fn unfreeze_requires_admin_auth() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let id = env.register(
        PropertyToken,
        (
            admin,
            String::from_str(&env, "prop-001"),
            String::from_str(&env, "Shares"),
            String::from_str(&env, "SHR"),
            100i128,
        ),
    );
    let client = PropertyTokenClient::new(&env, &id);
    // No mocked auths: the admin has not signed, so this must panic.
    client.unfreeze(&Address::generate(&env));
}

#[test]
fn recover_moves_shares_without_changing_supply() {
    let s = setup();
    let alice = investor(&s);
    let bob = investor(&s);
    s.client.mint(&alice, &100);
    let supply_before = s.client.total_supply();

    s.client.recover(&alice, &bob, &40);

    assert_eq!(s.client.balance(&alice), 60);
    assert_eq!(s.client.balance(&bob), 40);
    assert_eq!(s.client.total_supply(), supply_before);
}

#[test]
fn recover_to_non_whitelisted_fails() {
    let s = setup();
    let alice = investor(&s);
    let stranger = Address::generate(&s.env);
    s.client.mint(&alice, &100);
    assert_eq!(
        s.client.try_recover(&alice, &stranger, &10),
        Err(Ok(err(Error::NotWhitelisted)))
    );
    assert_eq!(s.client.balance(&alice), 100);
}

#[test]
#[should_panic]
fn recover_requires_admin_auth() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let id = env.register(
        PropertyToken,
        (
            admin,
            String::from_str(&env, "prop-001"),
            String::from_str(&env, "Shares"),
            String::from_str(&env, "SHR"),
            100i128,
        ),
    );
    let client = PropertyTokenClient::new(&env, &id);
    // No mocked auths: the admin has not signed, so this must panic.
    client.recover(&Address::generate(&env), &Address::generate(&env), &1);
}
