//! Rental yield distributor.
//!
//! The admin (the API's signing account) computes each holder's share of a
//! rental income period off-chain from a property-token balance snapshot and
//! calls `distribute` with those allocations. The payout asset (for example a
//! USDC Stellar Asset Contract) is pulled from the admin into this contract in
//! the same call, and holders withdraw their balance with `claim`.
#![no_std]

mod errors;
mod events;
mod storage;

#[cfg(test)]
mod proptests;
#[cfg(test)]
mod test;

pub use errors::Error;

use soroban_sdk::{
    contract, contractimpl, panic_with_error, token, Address, BytesN, Env, Map, Vec,
};
use storage::DataKey;

pub use storage::RoundInfo;

/// Upper bound on allocations per call, to stay well inside the resource limits.
pub const MAX_ALLOCATIONS: u32 = 100;

/// Crate version, exposed through the `version` view.
pub const VERSION: u32 = 1;

#[contract]
pub struct YieldDistributor;

#[contractimpl]
impl YieldDistributor {
    pub fn __constructor(
        env: Env,
        admin: Address,
        payout_token: Address,
        share_token: Address,
        claim_window_secs: u64,
    ) {
        let s = env.storage().instance();
        s.set(&DataKey::Admin, &admin);
        s.set(&DataKey::PayoutToken, &payout_token);
        s.set(&DataKey::ShareToken, &share_token);
        s.set(&DataKey::ClaimWindowSecs, &claim_window_secs);
        s.set(&DataKey::Round, &0u32);
        s.set(&DataKey::TotalDistributed, &0i128);
        s.set(&DataKey::TotalClaimed, &0i128);
        s.set(&DataKey::TotalReclaimed, &0i128);
    }

    /// Credits each holder with their allocation and pulls the total from the admin.
    /// Returns the new distribution round number.
    pub fn distribute(env: Env, allocations: Map<Address, i128>) -> u32 {
        let admin = storage::admin(&env);
        admin.require_auth();

        if allocations.is_empty() || allocations.len() > MAX_ALLOCATIONS {
            panic_with_error!(&env, Error::InvalidAllocations);
        }

        let share_token = token::Client::new(&env, &storage::share_token(&env));

        let mut total: i128 = 0;
        for (holder, amount) in allocations.iter() {
            if amount <= 0 {
                panic_with_error!(&env, Error::InvalidAmount);
            }
            if !share_token.is_whitelisted(&holder) {
                panic_with_error!(&env, Error::RecipientNotWhitelisted);
            }
            total = total
                .checked_add(amount)
                .unwrap_or_else(|| panic_with_error!(&env, Error::Overflow));
            let next = storage::claimable(&env, &holder)
                .checked_add(amount)
                .unwrap_or_else(|| panic_with_error!(&env, Error::Overflow));
            storage::set_claimable(&env, &holder, next);
            storage::set_last_credit(&env, &holder, env.ledger().timestamp());
        }

        token::Client::new(&env, &storage::payout_token(&env)).transfer(
            &admin,
            env.current_contract_address(),
            &total,
        );

        let round = storage::bump_round(&env);
        storage::add_total(&env, DataKey::TotalDistributed, total);
        let info = RoundInfo {
            total,
            recipients: allocations.len(),
            ledger: env.ledger().sequence(),
            timestamp: env.ledger().timestamp(),
        };
        storage::set_round_info(&env, round, &info);
        storage::extend_instance(&env);
        events::distributed(&env, round, total, allocations.len());
        round
    }

    /// Sends the holder everything credited to them so far. Returns the amount paid.
    pub fn claim(env: Env, holder: Address) -> i128 {
        holder.require_auth();
        Self::payout(&env, &holder, &holder)
    }

    /// Sends the holder everything credited to them so far to `destination`.
    /// Returns the amount paid.
    pub fn claim_to(env: Env, holder: Address, destination: Address) -> i128 {
        holder.require_auth();
        Self::payout(&env, &holder, &destination)
    }

    /// Reclaims a holder's unclaimed balance back to the admin once the claim
    /// window has elapsed since their last credit. Returns the amount recovered.
    pub fn reclaim(env: Env, holder: Address) -> i128 {
        let admin = storage::admin(&env);
        admin.require_auth();

        let amount = storage::claimable(&env, &holder);
        if amount == 0 {
            panic_with_error!(&env, Error::NothingToClaim);
        }

        let last_credit = storage::last_credit(&env, &holder);
        let deadline = last_credit
            .checked_add(storage::claim_window_secs(&env))
            .unwrap_or_else(|| panic_with_error!(&env, Error::Overflow));
        if env.ledger().timestamp() < deadline {
            panic_with_error!(&env, Error::ClaimWindowOpen);
        }

        storage::set_claimable(&env, &holder, 0);
        storage::add_total(&env, DataKey::TotalReclaimed, amount);

        token::Client::new(&env, &storage::payout_token(&env)).transfer(
            &env.current_contract_address(),
            &admin,
            &amount,
        );
        events::reclaimed(&env, holder, amount);
        amount
    }

    /// Proposes `new_admin` as the successor. The current admin stays in place
    /// until the proposed account calls `accept_admin`.
    pub fn propose_admin(env: Env, new_admin: Address) {
        storage::admin(&env).require_auth();
        env.storage().instance().set(&DataKey::PendingAdmin, &new_admin);
        events::admin_proposed(&env, new_admin);
    }

    /// Accepts a pending admin proposal. Must be called by the proposed account.
    pub fn accept_admin(env: Env) {
        let new_admin = storage::pending_admin(&env);
        new_admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        env.storage().instance().remove(&DataKey::PendingAdmin);
        events::admin_changed(&env, new_admin);
    }

    /// Cancels a pending admin proposal. Only the current admin may do this.
    pub fn cancel_admin_proposal(env: Env) {
        storage::admin(&env).require_auth();
        env.storage().instance().remove(&DataKey::PendingAdmin);
    }

    /// Extends the TTL of the instance and of each account's claimable entry.
    /// Permissionless: anyone may keep entries alive to avoid archival.
    pub fn bump(env: Env, accounts: Vec<Address>) {
        storage::bump(&env, &accounts);
    }

    /// Replaces this contract's code in place. Admin auth required.
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) {
        storage::admin(&env).require_auth();
        env.deployer()
            .update_current_contract_wasm(new_wasm_hash.clone());
        events::upgraded(&env, new_wasm_hash);
    }

    // Views

    pub fn version(_env: Env) -> u32 {
        VERSION
    }

    pub fn claimable(env: Env, holder: Address) -> i128 {
        storage::claimable(&env, &holder)
    }

    pub fn round(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::Round).unwrap_or(0)
    }

    pub fn round_info(env: Env, round: u32) -> Option<RoundInfo> {
        storage::round_info(&env, round)
    }

    pub fn total_distributed(env: Env) -> i128 {
        storage::total(&env, DataKey::TotalDistributed)
    }

    pub fn total_claimed(env: Env) -> i128 {
        storage::total(&env, DataKey::TotalClaimed)
    }

    pub fn total_reclaimed(env: Env) -> i128 {
        storage::total(&env, DataKey::TotalReclaimed)
    }

    pub fn claim_window_secs(env: Env) -> u64 {
        storage::claim_window_secs(&env)
    }

    pub fn payout_token(env: Env) -> Address {
        storage::payout_token(&env)
    }

    pub fn share_token(env: Env) -> Address {
        storage::share_token(&env)
    }

    pub fn admin(env: Env) -> Address {
        storage::admin(&env)
    }

    pub fn pending_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::PendingAdmin)
    }
}

impl YieldDistributor {
    /// Shared claim logic: zeroes `holder`'s balance and pays it out to `destination`.
    fn payout(env: &Env, holder: &Address, destination: &Address) -> i128 {
        let amount = storage::claimable(env, holder);
        if amount == 0 {
            panic_with_error!(env, Error::NothingToClaim);
        }
        storage::set_claimable(env, holder, 0);
        storage::add_total(env, DataKey::TotalClaimed, amount);

        token::Client::new(env, &storage::payout_token(env)).transfer(
            &env.current_contract_address(),
            destination,
            &amount,
        );
        events::claimed(env, holder.clone(), destination.clone(), amount);
        amount
    }
}
