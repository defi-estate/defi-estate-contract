use soroban_sdk::{contracttype, Address, Env, Vec};

/// About 30 days at 5 second ledgers.
const TTL_EXTEND_TO: u32 = 518_400;
const TTL_THRESHOLD: u32 = TTL_EXTEND_TO - 17_280;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    PropertyId,
    Name,
    Symbol,
    MaxSupply,
    TotalSupply,
    Balance(Address),
    Whitelisted(Address),
    Paused,
    PendingAdmin,
    Allowance(Address, Address),
    MaxPerHolder,
    Frozen(Address),
}

pub fn extend_instance(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
}

/// Extends the TTL of the contract instance and of each account's balance
/// entry. Accounts without a balance entry are skipped. No auth required.
pub fn bump(env: &Env, accounts: &Vec<Address>) {
    extend_instance(env);
    for account in accounts.iter() {
        let key = DataKey::Balance(account.clone());
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

pub fn total_supply(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::TotalSupply)
        .unwrap_or(0)
}

pub fn max_supply(env: &Env) -> i128 {
    env.storage().instance().get(&DataKey::MaxSupply).unwrap()
}

pub fn balance(env: &Env, id: &Address) -> i128 {
    let key = DataKey::Balance(id.clone());
    match env.storage().persistent().get(&key) {
        Some(b) => {
            env.storage()
                .persistent()
                .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
            b
        }
        None => 0,
    }
}

pub fn set_balance(env: &Env, id: &Address, amount: i128) {
    let key = DataKey::Balance(id.clone());
    env.storage().persistent().set(&key, &amount);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

pub fn is_whitelisted(env: &Env, id: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::Whitelisted(id.clone()))
        .unwrap_or(false)
}

pub fn set_whitelisted(env: &Env, id: &Address, allowed: bool) {
    let key = DataKey::Whitelisted(id.clone());
    if allowed {
        env.storage().persistent().set(&key, &true);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
    } else {
        env.storage().persistent().remove(&key);
    }
}

pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false)
}

pub fn set_paused(env: &Env, paused: bool) {
    env.storage().instance().set(&DataKey::Paused, &paused);
}

pub fn max_per_holder(env: &Env) -> Option<i128> {
    env.storage().instance().get(&DataKey::MaxPerHolder)
}

pub fn set_max_per_holder(env: &Env, limit: Option<i128>) {
    match limit {
        Some(value) => env.storage().instance().set(&DataKey::MaxPerHolder, &value),
        None => env.storage().instance().remove(&DataKey::MaxPerHolder),
    }
}

pub fn is_frozen(env: &Env, id: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::Frozen(id.clone()))
        .unwrap_or(false)
}

pub fn set_frozen(env: &Env, id: &Address, frozen: bool) {
    let key = DataKey::Frozen(id.clone());
    if frozen {
        env.storage().persistent().set(&key, &true);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
    } else {
        env.storage().persistent().remove(&key);
    }
}

pub fn pending_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::PendingAdmin)
}

pub fn set_pending_admin(env: &Env, id: &Address) {
    env.storage().instance().set(&DataKey::PendingAdmin, id);
}

pub fn clear_pending_admin(env: &Env) {
    env.storage().instance().remove(&DataKey::PendingAdmin);
}

pub fn allowance(env: &Env, from: &Address, spender: &Address) -> i128 {
    let key = DataKey::Allowance(from.clone(), spender.clone());
    let (amount, expiration_ledger): (i128, u32) = env
        .storage()
        .temporary()
        .get(&key)
        .unwrap_or((0, 0));
    if expiration_ledger < env.ledger().sequence() {
        0
    } else {
        amount
    }
}

pub fn approve(
    env: &Env,
    from: &Address,
    spender: &Address,
    amount: i128,
    expiration_ledger: u32,
) {
    let key = DataKey::Allowance(from.clone(), spender.clone());
    env.storage()
        .temporary()
        .set(&key, &(amount, expiration_ledger));
    if amount > 0 && expiration_ledger > env.ledger().sequence() {
        env.storage().temporary().extend_ttl(
            &key,
            expiration_ledger - env.ledger().sequence(),
            expiration_ledger - env.ledger().sequence(),
        );
    }
}

pub fn spend_allowance(env: &Env, from: &Address, spender: &Address, amount: i128) {
    let key = DataKey::Allowance(from.clone(), spender.clone());
    let (current, expiration_ledger): (i128, u32) = env
        .storage()
        .temporary()
        .get(&key)
        .unwrap_or((0, 0));
    if expiration_ledger < env.ledger().sequence() || current < amount {
        panic_with_error!(env, crate::errors::Error::InsufficientAllowance);
    }
    env.storage()
        .temporary()
        .set(&key, &(current - amount, expiration_ledger));
}
