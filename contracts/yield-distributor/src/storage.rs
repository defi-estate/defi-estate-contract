use soroban_sdk::{contracttype, Address, Env, Vec};

/// About 30 days at 5 second ledgers.
const TTL_EXTEND_TO: u32 = 518_400;
const TTL_THRESHOLD: u32 = TTL_EXTEND_TO - 17_280;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    PendingAdmin,
    PayoutToken,
    ShareToken,
    Round,
    RoundInfo(u32),
    TotalDistributed,
    TotalClaimed,
    TotalReclaimed,
    ClaimWindowSecs,
    Claimable(Address),
    LastCredit(Address),
}

/// Summary of a single distribution round, kept for audits.
#[contracttype]
#[derive(Clone)]
pub struct RoundInfo {
    pub total: i128,
    pub recipients: u32,
    pub ledger: u32,
    pub timestamp: u64,
}

pub fn extend_instance(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
}

/// Extends the TTL of the instance storage and of the claimable entry for
/// each account that has one. Accounts without a claimable entry are skipped.
pub fn bump(env: &Env, accounts: &Vec<Address>) {
    extend_instance(env);
    for account in accounts.iter() {
        let key = DataKey::Claimable(account.clone());
        if env.storage().persistent().has(&key) {
            env.storage()
                .persistent()
                .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
        }
    }
}

pub fn admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).unwrap()
}

pub fn pending_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::PendingAdmin)
}

pub fn set_pending_admin(env: &Env, new_admin: &Address) {
    env.storage()
        .instance()
        .set(&DataKey::PendingAdmin, new_admin);
}

pub fn clear_pending_admin(env: &Env) {
    env.storage().instance().remove(&DataKey::PendingAdmin);
}

pub fn payout_token(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::PayoutToken).unwrap()
}

pub fn bump_round(env: &Env) -> u32 {
    let round: u32 = env.storage().instance().get(&DataKey::Round).unwrap_or(0) + 1;
    env.storage().instance().set(&DataKey::Round, &round);
    round
}

pub fn total(env: &Env, key: DataKey) -> i128 {
    env.storage().instance().get(&key).unwrap_or(0)
}

pub fn add_total(env: &Env, key: DataKey, amount: i128) {
    let next = total(env, key.clone()) + amount;
    env.storage().instance().set(&key, &next);
}

pub fn set_round_info(env: &Env, round: u32, info: &RoundInfo) {
    let key = DataKey::RoundInfo(round);
    env.storage().persistent().set(&key, info);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

pub fn round_info(env: &Env, round: u32) -> Option<RoundInfo> {
    let key = DataKey::RoundInfo(round);
    let info: Option<RoundInfo> = env.storage().persistent().get(&key);
    if info.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
    }
    info
}

pub fn claimable(env: &Env, holder: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::Claimable(holder.clone()))
        .unwrap_or(0)
}

pub fn set_claimable(env: &Env, holder: &Address, amount: i128) {
    let key = DataKey::Claimable(holder.clone());
    if amount == 0 {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, &amount);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
    }
}

pub fn claim_window_secs(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::ClaimWindowSecs)
        .unwrap_or(0)
}

pub fn set_claim_window_secs(env: &Env, secs: u64) {
    env.storage()
        .instance()
        .set(&DataKey::ClaimWindowSecs, &secs);
}

pub fn last_credit(env: &Env, holder: &Address) -> u64 {
    let key = DataKey::LastCredit(holder.clone());
    let timestamp: Option<u64> = env.storage().persistent().get(&key);
    match timestamp {
        Some(ts) => {
            env.storage()
                .persistent()
                .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
            ts
        }
        None => 0,
    }
}

pub fn set_last_credit(env: &Env, holder: &Address, timestamp: u64) {
    let key = DataKey::LastCredit(holder.clone());
    env.storage().persistent().set(&key, &timestamp);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

pub fn clear_last_credit(env: &Env, holder: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::LastCredit(holder.clone()));
}
