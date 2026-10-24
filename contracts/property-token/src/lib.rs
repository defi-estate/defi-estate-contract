//! Property share token.
//!
//! One instance per property. The supply is capped at the number of shares the
//! property was split into, only the admin can mint, and shares can only be held
//! by whitelisted (KYC-reviewed) accounts. Shares are whole units (0 decimals).
#![no_std]

mod errors;
mod events;
mod storage;

#[cfg(test)]
mod proptests;
#[cfg(test)]
mod test;

pub use errors::Error;

use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env, String, Vec};
use storage::DataKey;

/// Contract version, bumped on every upgrade.
pub const VERSION: u32 = 1;

#[contract]
pub struct PropertyToken;

#[contractimpl]
impl PropertyToken {
    /// Sets the admin, token metadata and the maximum number of shares.
    /// `property_id` is the off-chain identifier (the API's property UUID).
    pub fn __constructor(
        env: Env,
        admin: Address,
        property_id: String,
        name: String,
        symbol: String,
        max_supply: i128,
    ) {
        if max_supply <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let s = env.storage().instance();
        s.set(&DataKey::Admin, &admin);
        s.set(&DataKey::PropertyId, &property_id);
        s.set(&DataKey::Name, &name);
        s.set(&DataKey::Symbol, &symbol);
        s.set(&DataKey::MaxSupply, &max_supply);
        s.set(&DataKey::TotalSupply, &0i128);
    }

    // Admin

    pub fn set_whitelisted(env: Env, account: Address, allowed: bool) {
        storage::admin(&env).require_auth();
        storage::set_whitelisted(&env, &account, allowed);
        events::whitelist_updated(&env, account, allowed);
    }

    /// Sets (or clears, with `None`) the maximum number of shares a single
    /// holder may own. Admin only.
    pub fn set_max_per_holder(env: Env, max_per_holder: Option<i128>) {
        storage::admin(&env).require_auth();
        if let Some(limit) = max_per_holder {
            if limit <= 0 {
                panic_with_error!(&env, Error::InvalidAmount);
            }
        }
        match &max_per_holder {
            Some(limit) => env.storage().instance().set(&DataKey::MaxPerHolder, limit),
            None => env.storage().instance().remove(&DataKey::MaxPerHolder),
        }
        events::max_per_holder_updated(&env, max_per_holder);
    }

    /// Freezes a single account: it can neither send nor receive shares.
    /// Admin only.
    pub fn freeze(env: Env, account: Address) {
        storage::admin(&env).require_auth();
        storage::set_frozen(&env, &account, true);
        events::account_frozen(&env, account);
    }

    /// Unfreezes a previously frozen account. Admin only.
    pub fn unfreeze(env: Env, account: Address) {
        storage::admin(&env).require_auth();
        storage::set_frozen(&env, &account, false);
        events::account_unfrozen(&env, account);
    }

    /// Moves shares from `from` to `to` without `from`'s authorization, for
    /// investors who lost access to their wallet. Admin only; `to` must be
    /// whitelisted. Supply is unchanged.
    pub fn recover(env: Env, from: Address, to: Address, amount: i128) {
        storage::admin(&env).require_auth();
        require_positive(&env, amount);
        require_whitelisted(&env, &to);
        require_not_frozen(&env, &to);
        require_holder_limit(&env, &to, amount);

        move_shares(&env, &from, &to, amount);
        events::recovered(&env, from, to, amount);
    }

    pub fn mint(env: Env, to: Address, amount: i128) {
        storage::admin(&env).require_auth();
        require_not_paused(&env);
        require_positive(&env, amount);
        require_whitelisted(&env, &to);
        require_not_frozen(&env, &to);
        require_holder_limit(&env, &to, amount);

        let supply = storage::total_supply(&env)
            .checked_add(amount)
            .unwrap_or_else(|| panic_with_error!(&env, Error::Overflow));
        if supply > storage::max_supply(&env) {
            panic_with_error!(&env, Error::MaxSupplyExceeded);
        }
        env.storage().instance().set(&DataKey::TotalSupply, &supply);
        storage::set_balance(&env, &to, storage::balance(&env, &to) + amount);
        storage::extend_instance(&env);
        events::mint(&env, to, amount);
    }

    pub fn burn(env: Env, from: Address, amount: i128) {
        storage::admin(&env).require_auth();
        require_positive(&env, amount);
        burn_shares(&env, &from, amount);
    }

    pub fn pause(env: Env) {
        storage::admin(&env).require_auth();
        env.storage().instance().set(&DataKey::Paused, &true);
        events::paused(&env);
    }

    pub fn unpause(env: Env) {
        storage::admin(&env).require_auth();
        env.storage().instance().set(&DataKey::Paused, &false);
        events::unpaused(&env);
    }

    pub fn propose_admin(env: Env, new_admin: Address) {
        storage::admin(&env).require_auth();
        env.storage().instance().set(&DataKey::PendingAdmin, &new_admin);
        events::admin_proposed(&env, new_admin);
    }

    pub fn accept_admin(env: Env) {
        let pending = storage::pending_admin(&env)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NoPendingAdmin));
        pending.require_auth();
        env.storage().instance().set(&DataKey::Admin, &pending);
        env.storage().instance().remove(&DataKey::PendingAdmin);
        events::admin_changed(&env, pending);
    }

    pub fn cancel_admin_proposal(env: Env) {
        storage::admin(&env).require_auth();
        env.storage().instance().remove(&DataKey::PendingAdmin);
    }

    /// Replaces the contract code in place. Admin only.
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) {
        storage::admin(&env).require_auth();
        env.deployer()
            .update_current_contract_wasm(new_wasm_hash.clone());
        events::upgraded(&env, new_wasm_hash);
    }

    // Holders

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        require_not_paused(&env);
        require_positive(&env, amount);
        require_whitelisted(&env, &to);
        require_not_frozen(&env, &from);
        require_not_frozen(&env, &to);
        require_holder_limit(&env, &to, amount);

        move_shares(&env, &from, &to, amount);
        events::transfer(&env, from, to, amount);
    }

    pub fn approve(env: Env, from: Address, spender: Address, amount: i128, expiration_ledger: u32) {
        from.require_auth();
        if amount < 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        storage::set_allowance(&env, &from, &spender, amount, expiration_ledger);
        events::approve(&env, from, spender, amount, expiration_ledger);
    }

    pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();
        require_not_paused(&env);
        require_positive(&env, amount);
        require_whitelisted(&env, &to);
        require_not_frozen(&env, &from);
        require_not_frozen(&env, &to);
        require_holder_limit(&env, &to, amount);

        storage::spend_allowance(&env, &from, &spender, amount);
        move_shares(&env, &from, &to, amount);
        events::transfer(&env, from, to, amount);
    }

    pub fn burn_from(env: Env, spender: Address, from: Address, amount: i128) {
        spender.require_auth();
        require_positive(&env, amount);

        storage::spend_allowance(&env, &from, &spender, amount);
        burn_shares(&env, &from, amount);
    }

    /// Extends the TTL of the contract instance and of each account's balance
    /// entry. Permissionless: anyone can keep entries from being archived.
    pub fn bump(env: Env, accounts: Vec<Address>) {
        storage::bump(&env, &accounts);
    }

    // Views

    pub fn balance(env: Env, id: Address) -> i128 {
        storage::balance(&env, &id)
    }

    pub fn total_supply(env: Env) -> i128 {
        storage::total_supply(&env)
    }

    pub fn max_supply(env: Env) -> i128 {
        storage::max_supply(&env)
    }

    pub fn is_whitelisted(env: Env, account: Address) -> bool {
        storage::is_whitelisted(&env, &account)
    }

    pub fn max_per_holder(env: Env) -> Option<i128> {
        storage::max_per_holder(&env)
    }

    pub fn is_frozen(env: Env, account: Address) -> bool {
        storage::is_frozen(&env, &account)
    }

    pub fn admin(env: Env) -> Address {
        storage::admin(&env)
    }

    pub fn pending_admin(env: Env) -> Option<Address> {
        storage::pending_admin(&env)
    }

    pub fn allowance(env: Env, from: Address, spender: Address) -> i128 {
        storage::allowance(&env, &from, &spender)
    }

    pub fn is_paused(env: Env) -> bool {
        storage::is_paused(&env)
    }

    pub fn property_id(env: Env) -> String {
        env.storage().instance().get(&DataKey::PropertyId).unwrap()
    }

    pub fn name(env: Env) -> String {
        env.storage().instance().get(&DataKey::Name).unwrap()
    }

    pub fn symbol(env: Env) -> String {
        env.storage().instance().get(&DataKey::Symbol).unwrap()
    }

    pub fn decimals(_env: Env) -> u32 {
        0
    }

    pub fn version(_env: Env) -> u32 {
        VERSION
    }
}

fn require_positive(env: &Env, amount: i128) {
    if amount <= 0 {
        panic_with_error!(env, Error::InvalidAmount);
    }
}

fn require_whitelisted(env: &Env, account: &Address) {
    if !storage::is_whitelisted(env, account) {
        panic_with_error!(env, Error::NotWhitelisted);
    }
}

fn require_not_paused(env: &Env) {
    if storage::is_paused(env) {
        panic_with_error!(env, Error::Paused);
    }
}

fn require_not_frozen(env: &Env, account: &Address) {
    if storage::is_frozen(env, account) {
        panic_with_error!(env, Error::AccountFrozen);
    }
}

/// Ensures crediting `amount` to `account` keeps it within the configured
/// per-holder limit, if any.
fn require_holder_limit(env: &Env, account: &Address, amount: i128) {
    if let Some(limit) = storage::max_per_holder(env) {
        let new_balance = storage::balance(env, account)
            .checked_add(amount)
            .unwrap_or_else(|| panic_with_error!(env, Error::Overflow));
        if new_balance > limit {
            panic_with_error!(env, Error::HolderLimitExceeded);
        }
    }
}

fn move_shares(env: &Env, from: &Address, to: &Address, amount: i128) {
    let from_balance = storage::balance(env, from);
    if from_balance < amount {
        panic_with_error!(env, Error::InsufficientBalance);
    }
    storage::set_balance(env, from, from_balance - amount);
    storage::set_balance(env, to, storage::balance(env, to) + amount);
}

fn burn_shares(env: &Env, from: &Address, amount: i128) {
    let from_balance = storage::balance(env, from);
    if from_balance < amount {
        panic_with_error!(env, Error::InsufficientBalance);
    }
    storage::set_balance(env, from, from_balance - amount);
    let supply = storage::total_supply(env) - amount;
    env.storage().instance().set(&DataKey::TotalSupply, &supply);
    storage::extend_instance(env);
    events::burn(env, from.clone(), amount);
}
