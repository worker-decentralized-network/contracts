use candid::{candid_method, Nat};
use ic_cdk::{
    api::call::CallResult,
    export::{
        candid::{CandidType, Deserialize},
        Principal,
    },
};
use ic_cdk_macros::*;
use ic_ledger_types::{
    query_archived_blocks, query_blocks, AccountIdentifier, Block, BlockIndex, GetBlocksArgs,
    Operation, DEFAULT_SUBACCOUNT,
};
use std::{cell::RefCell, collections::BTreeMap, collections::HashSet};

type ProfileStore = BTreeMap<Principal, Profile>;
type ChargeBlock = HashSet<BlockIndex>;

#[derive(Clone, Debug, CandidType, Deserialize, PartialEq)]
enum ProfileStatus {
    Pending,
    Passed,
    Refused,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
struct Profile {
    pub community_name: String,
    pub discord_account: String,
    pub wallet_address: Principal,
    pub status: ProfileStatus,
    pub comment: String,
    pub number: u16,
}

#[derive(Deserialize, CandidType, Clone, Debug)]
struct StatsData {
    owner: Principal,
    admin: Principal,
    closed: bool,

    ledger: Principal,
    fee: Nat,
    fee_to: Principal,
}

impl Default for StatsData {
    fn default() -> Self {
        StatsData {
            owner: Principal::anonymous(),
            admin: Principal::anonymous(),
            closed: false,

            ledger: Principal::anonymous(),
            fee: Nat::from(0),
            fee_to: Principal::anonymous(),
        }
    }
}

thread_local! {
    static STATS: RefCell<StatsData> = RefCell::new(StatsData::default());
    static PROFILE_STORE: RefCell<ProfileStore> = RefCell::new(ProfileStore::default());
    static CHARGE_BLOCK: RefCell<ChargeBlock> = RefCell::new(ChargeBlock::default());
    static LAST_NUMBER: RefCell<u16> = RefCell::new(0);
}

#[init]
#[candid_method(init)]
fn init() {
    STATS.with(|s| {
        let mut stats = s.borrow_mut();
        stats.owner = ic_cdk::api::caller();
        stats.closed = false;
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

#[update(name = "setLedger", guard = "_is_auth")]
#[candid_method(update, rename = "setLedger")]
fn set_ledger(ledger: Principal) -> Result<(), String> {
    STATS.with(|s| {
        let mut stats = s.borrow_mut();
        stats.ledger = ledger;
        Ok(())
    })
}

#[update(name = "setFee", guard = "_is_auth")]
#[candid_method(update, rename = "setFee")]
fn set_fee(fee: Nat) -> Result<(), String> {
    STATS.with(|s| {
        let mut stats = s.borrow_mut();
        stats.fee = fee;
        Ok(())
    })
}

#[update(name = "setFeeTo", guard = "_is_auth")]
#[candid_method(update, rename = "setFeeTo")]
fn set_fee_to(fee_to: Principal) -> Result<(), String> {
    STATS.with(|s| {
        let mut stats = s.borrow_mut();
        stats.fee_to = fee_to;
        Ok(())
    })
}

#[update(name = "setAdmin", guard = "_is_auth")]
#[candid_method(update, rename = "setAdmin")]
fn set_admin(admin: Principal) -> Result<(), String> {
    STATS.with(|s| {
        let mut stats = s.borrow_mut();
        stats.admin = admin;
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

#[query(name = "get")]
#[candid_method(query)]
fn get() -> Result<Profile, String> {
    let caller = ic_cdk::api::caller();
    PROFILE_STORE.with(|profile_store| {
        let store = profile_store.borrow();
        let profile = store.get(&caller);
        match profile {
            Some(p) => Ok(p.clone()),
            None => Err("not found".to_string()),
        }
    })
}

#[update]
#[candid_method(update)]
async fn insert(
    block_index: BlockIndex,
    community_name: String,
    discord_account: String,
) -> Result<(), String> {
    let caller = ic_cdk::api::caller();

    let (ledger, fee) = STATS.with(|s| {
        let stats = s.borrow();
        (stats.ledger, stats.fee.clone())
    });

    let amount = check_charge_block(ledger, block_index, caller).await?;
    if amount < fee {
        return Err("invalid fee".to_string());
    }

    let mut res = Ok(());

    STATS.with(|s| {
        let stats = s.borrow();
        if stats.closed {
            res = Err("closed".to_string());
        }
    });

    if res.is_err() {
        return res;
    }

    PROFILE_STORE.with(|profile_store| {
        let mut store = profile_store.borrow_mut();
        let info = store.get_mut(&caller);
        match info {
            Some(info) => {
                if info.status == ProfileStatus::Refused {
                    res = save_charge_block(block_index);
                    if res.is_err() {
                        return;
                    }

                    info.community_name = community_name;
                    info.discord_account = discord_account;
                    info.wallet_address = caller;
                    info.status = ProfileStatus::Pending;
                    info.comment = "".to_string();
                    info.number = 0;
                } else {
                    res = Err("duplicate wallet address".to_string());
                    return;
                }
            }
            None => {
                res = save_charge_block(block_index);
                if res.is_err() {
                    return;
                }

                store.insert(
                    caller.clone(),
                    Profile {
                        community_name,
                        discord_account,
                        wallet_address: caller,
                        status: ProfileStatus::Pending,
                        comment: "".to_string(),
                        number: 0,
                    },
                );
            }
        }
    });

    res
}

#[update(name = "pass", guard = "_is_admin")]
#[candid_method(update)]
fn pass(account: Principal) -> Result<(), String> {
    let last_number = LAST_NUMBER.with(|n| n.borrow().clone());
    let last_number = last_number + 1;

    let res = PROFILE_STORE.with(|profile_store| {
        let mut store = profile_store.borrow_mut();

        if let Some(p) = store.get_mut(&account) {
            p.status = ProfileStatus::Passed;
            p.number = last_number;
            Ok(())
        } else {
            Err("wallet address not found".to_string())
        }
    });
    if res.is_err() {
        return res;
    }

    LAST_NUMBER.with(|n| {
        let mut n = n.borrow_mut();
        *n = last_number;
    });

    Ok(())
}

#[update(name = "refuse", guard = "_is_admin")]
#[candid_method(update)]
fn refuse(account: Principal, comment: String) -> Result<(), String> {
    PROFILE_STORE.with(|profile_store| {
        let mut store = profile_store.borrow_mut();

        if let Some(p) = store.get_mut(&account) {
            p.status = ProfileStatus::Refused;
            p.comment = comment;
            Ok(())
        } else {
            Err("wallet address not found".to_string())
        }
    })
}

#[update(name = "updateProfile", guard = "_is_admin")]
#[candid_method(update, rename = "updateProfile")]
fn update_profile(profile: Profile) -> Result<(), String> {
    PROFILE_STORE.with(|profile_store| {
        let mut store = profile_store.borrow_mut();

        if let Some(p) = store.get_mut(&profile.wallet_address) {
            *p = profile;
            Ok(())
        } else {
            Err("wallet address not found".to_string())
        }
    })
}

#[query(name = "getAll", guard = "_is_admin")]
#[candid_method(query, rename = "getAll")]
fn get_all(start: usize, limit: usize) -> Vec<Profile> {
    let mut count = 0;
    PROFILE_STORE.with(|profile_store| {
        let mut profiles = Vec::new();
        for (_, p) in profile_store.borrow().iter() {
            if count < start {
                count += 1;
                continue;
            }
            if profiles.len() >= limit {
                break;
            }
            profiles.push(p.clone());
            count += 1;
        }
        profiles
    })
}

#[pre_upgrade]
fn pre_upgrade() {
    let stats = STATS.with(|s| s.borrow().clone());
    let profile_store = PROFILE_STORE.with(|a| a.borrow().clone());
    let charge_block = CHARGE_BLOCK.with(|a| a.borrow().clone());
    let last_number = LAST_NUMBER.with(|a| a.borrow().clone());
    ic_cdk::storage::stable_save((stats, profile_store, charge_block, last_number)).unwrap();
}

#[post_upgrade]
fn post_upgrade() {
    let (stats_stored, profile_stored, charge_block_stored, last_number_stored) =
        ic_cdk::storage::stable_restore().unwrap();
    STATS.with(|s| {
        let mut stats = s.borrow_mut();
        *stats = stats_stored;
    });
    PROFILE_STORE.with(|p| {
        let mut profile_store = p.borrow_mut();
        *profile_store = profile_stored;
    });
    CHARGE_BLOCK.with(|p| {
        let mut charge_block = p.borrow_mut();
        *charge_block = charge_block_stored;
    });
    LAST_NUMBER.with(|p| {
        let mut last_number = p.borrow_mut();
        *last_number = last_number_stored;
    });
}

async fn query_one_block(ledger: Principal, block_index: BlockIndex) -> CallResult<Option<Block>> {
    let args = GetBlocksArgs {
        start: block_index,
        length: 1,
    };

    let blocks_result = query_blocks(ledger, args.clone()).await?;

    if blocks_result.blocks.len() >= 1 {
        debug_assert_eq!(blocks_result.first_block_index, block_index);
        return Ok(blocks_result.blocks.into_iter().next());
    }

    if let Some(func) = blocks_result.archived_blocks.into_iter().find_map(|b| {
        (b.start <= block_index && (block_index - b.start) < b.length).then(|| b.callback)
    }) {
        match query_archived_blocks(&func, args).await? {
            Ok(range) => return Ok(range.blocks.into_iter().next()),
            _ => (),
        }
    }
    Ok(None)
}

async fn check_charge_block(
    ledger: Principal,
    block_index: BlockIndex,
    caller: Principal,
) -> Result<u64, String> {
    let block_used = CHARGE_BLOCK.with(|a| {
        let blocks = a.borrow();
        blocks.contains(&block_index)
    });
    if block_used {
        return Err("block used".to_string());
    }

    let blocks_result = query_one_block(ledger, block_index).await;
    let block = match blocks_result {
        Ok(Some(block)) => block,
        Ok(None) => return Err("block not found".to_string()),
        Err(e) => return Err(e.1),
    };

    let op = match block.transaction.operation {
        Some(o) => o,
        None => return Err("invalid block".to_string()),
    };
    let (from, to, amount) = match op {
        Operation::Transfer {
            from,
            to,
            amount,
            fee: _,
        } => (from, to, amount),
        _ => return Err("invalid block".to_string()),
    };

    let caller_id = AccountIdentifier::new(&caller, &DEFAULT_SUBACCOUNT);
    if from != caller_id {
        return Err("invalid transaction".to_string());
    }
    let fee_to = STATS.with(|s| {
        let stats = s.borrow();
        stats.fee_to
    });
    let fee_to_id = AccountIdentifier::new(&fee_to, &DEFAULT_SUBACCOUNT);
    if to != fee_to_id {
        return Err("invalid transaction".to_string());
    }
    Ok(amount.e8s())
}

fn save_charge_block(block_index: BlockIndex) -> Result<(), String> {
    CHARGE_BLOCK.with(|a| {
        let mut block = a.borrow_mut();
        if block.insert(block_index) {
            Ok(())
        } else {
            Err("block_index already used".to_string())
        }
    })
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

fn _is_admin() -> Result<(), String> {
    STATS.with(|s| {
        let stats = s.borrow();
        if ic_cdk::api::caller() == stats.admin {
            Ok(())
        } else {
            Err("invalid caller".to_string())
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
