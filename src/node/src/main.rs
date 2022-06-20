use candid::{candid_method, Nat};
use ic_cdk::{
    api::call::CallResult,
    export::{
        candid::{CandidType, Deserialize},
        Principal,
    },
};
use ic_cdk_macros::*;
use std::cell::RefCell;
use std::collections::HashSet;

#[derive(CandidType, Debug, PartialEq, Deserialize)]
pub enum TxError {
    InsufficientBalance,
    InsufficientAllowance,
    Unauthorized,
    LedgerTrap,
    AmountTooSmall,
    BlockUsed,
    ErrorOperationStyle,
    ErrorTo,
    Other(String),
}
pub type TxReceipt = Result<Nat, TxError>;

type Keeper = HashSet<Principal>;
type Backer = HashSet<Principal>;

#[derive(Clone, Debug, Deserialize, CandidType)]
struct StatsData {
    owner: Principal,
    closed: bool,
    token: Principal,
}

impl Default for StatsData {
    fn default() -> Self {
        StatsData {
            owner: Principal::anonymous(),
            closed: false,
            token: Principal::anonymous(),
        }
    }
}

thread_local! {
    static STATS: RefCell<StatsData> = RefCell::new(StatsData::default());
    static KEEPERS: RefCell<Keeper> = RefCell::new(Keeper::default());
    static BACKERS: RefCell<Backer> = RefCell::new(Backer::default());
}

#[init]
#[candid_method(init)]
fn init(token: Principal, official_keeper: Principal) {
    STATS.with(|s| {
        let mut stats = s.borrow_mut();
        stats.owner = ic_cdk::api::caller();
        stats.closed = false;
        stats.token = token;
    });
    KEEPERS.with(|k| {
        let mut keepers = k.borrow_mut();
        keepers.insert(official_keeper);
    });
}

#[update(name = "setClosed", guard = "_is_auth")]
#[candid_method(update, rename = "setClosed")]
fn set_closed(closed: bool) -> Result<(), String> {
    STATS.with(|s| {
        let mut stats = s.borrow_mut();
        stats.closed = closed;
        Ok(())
    })
}

#[query(name = "getStats")]
#[candid_method(query, rename = "getStats")]
fn get_stats() -> Result<StatsData, String> {
    STATS.with(|s| {
        let stats = s.borrow();
        Ok(stats.clone())
    })
}

#[query(name = "getKeepers")]
#[candid_method(query, rename = "getKeepers")]
fn get_keepers() -> Keeper {
    KEEPERS.with(|k| {
        let keepers = k.borrow();
        keepers.clone()
    })
}

#[query(name = "getBackers")]
#[candid_method(query, rename = "getBackers")]
fn get_backers() -> Backer {
    BACKERS.with(|b| {
        let backers = b.borrow();
        backers.clone()
    })
}

// there is only one keeper now
#[update(guard = "_is_closed")]
#[candid_method(update)]
async fn withdraw(worker: Principal, amount: Nat) -> Result<Nat, String> {
    let caller = ic_cdk::caller();
    let is_keeper = KEEPERS.with(|k| {
        let keepers = k.borrow();
        keepers.contains(&caller)
    });

    if !is_keeper {
        return Err("invalid caller".to_string());
    }

    let token = STATS.with(|s| s.borrow().token);

    let call_result: CallResult<(TxReceipt,)> = ic_cdk::call(token, "mint", (worker, amount)).await;
    match call_result {
        Ok(tx) => match tx {
            (Ok(tx_id),) => Ok(tx_id),
            (Err(e),) => Err(format!("{:?}", e)),
        },
        Err(e) => Err(format!("{:?} {}", e.0, e.1)),
    }
}

#[pre_upgrade]
fn pre_upgrade() {
    let stats = STATS.with(|s| s.borrow().clone());
    let keepers = KEEPERS.with(|a| a.borrow().clone());
    let backers = BACKERS.with(|a| a.borrow().clone());
    ic_cdk::storage::stable_save((stats, keepers, backers)).unwrap();
}

#[post_upgrade]
fn post_upgrade() {
    let (stats_stored, keepers_stored, backers_stored) = ic_cdk::storage::stable_restore().unwrap();
    STATS.with(|s| {
        let mut stats = s.borrow_mut();
        *stats = stats_stored;
    });
    KEEPERS.with(|a| {
        let mut store = a.borrow_mut();
        *store = keepers_stored;
    });
    BACKERS.with(|a| {
        let mut store = a.borrow_mut();
        *store = backers_stored;
    });
}

fn _is_auth() -> Result<(), String> {
    STATS.with(|s| {
        let stats = s.borrow();
        if ic_cdk::api::caller() == stats.owner {
            Ok(())
        } else {
            Err("invalid caller".to_string())
        }
    })
}

fn _is_closed() -> Result<(), String> {
    STATS.with(|s| {
        let stats = s.borrow();
        if !stats.closed {
            Ok(())
        } else {
            Err("closed".to_string())
        }
    })
}

#[cfg(not(any(target_arch = "wasm32", test)))]
fn main() {
    // The line below generates did types and service definition from the
    // methods annotated with `candid_method` above. The definition is then
    // obtained with `__export_service()`.
    candid::export_service!();
    std::print!("{}", __export_service());
}

#[cfg(any(target_arch = "wasm32", test))]
fn main() {}
