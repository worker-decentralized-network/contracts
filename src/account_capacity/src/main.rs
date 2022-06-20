use candid::{candid_method, Nat};
use ic_cdk::{
    api::{call::CallResult, time},
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
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

type AccountCapacityInfo = HashMap<Principal, CapacityInfo>;
type ChargeBlock = HashSet<BlockIndex>;

#[derive(Clone, Debug, Deserialize, CandidType)]
struct CapacityInfo {
    pub inviter: Principal,

    // long term capacity
    pub long_term_capacity: u64,
    // reward capacity
    pub reward_capacity: u64,

    pub invitee_count: u64,
    pub invitation_expire: u64,

    pub charged: Nat,
    pub rest_charged: Nat,
}

impl Default for CapacityInfo {
    fn default() -> Self {
        CapacityInfo {
            inviter: Principal::anonymous(),
            long_term_capacity: 0,
            reward_capacity: 0,
            invitee_count: 0,
            invitation_expire: 0,
            charged: Nat::from(0),
            rest_charged: Nat::from(0),
        }
    }
}

#[derive(Clone, Debug, Deserialize, CandidType)]
struct StatsData {
    owner: Principal,
    closed: bool,
    fee: Nat,
    fee_with_inviter: Nat,
    fee_to: Principal,

    base_expire: u64,
    extend_expire: u64,

    ledger: Principal,
}

impl Default for StatsData {
    fn default() -> Self {
        StatsData {
            ledger: Principal::anonymous(),
            owner: Principal::anonymous(),
            closed: false,
            fee: Nat::from(0),
            fee_with_inviter: Nat::from(0),
            fee_to: Principal::anonymous(),

            base_expire: 0,
            extend_expire: 0,
        }
    }
}

static INVITE_REWARD: [(u64, u64); 5] = [(1, 1), (2, 1), (3, 2), (5, 3), (7, 6)];

thread_local! {
    static STATS: RefCell<StatsData> = RefCell::new(StatsData::default());
    static CAPACITY_INFO: RefCell<AccountCapacityInfo> = RefCell::new(AccountCapacityInfo::default());
    static CHARGE_BLOCK: RefCell<ChargeBlock> = RefCell::new(ChargeBlock::default());
}

#[init]
#[candid_method(init)]
fn init(
    ledger: Principal,
    fee: Nat,
    fee_with_inviter: Nat,
    fee_to: Principal,
    base_expire: u64,
    extend_expire: u64,
) {
    STATS.with(|s| {
        let mut stats = s.borrow_mut();
        stats.ledger = ledger;
        stats.owner = ic_cdk::api::caller();
        stats.closed = false;
        stats.fee = fee;
        stats.fee_with_inviter = fee_with_inviter;
        stats.fee_to = fee_to;
        stats.base_expire = base_expire;
        stats.extend_expire = extend_expire;
    });
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

#[update(name = "setFeeWithInviter", guard = "_is_auth")]
#[candid_method(update, rename = "setFeeWithInviter")]
fn set_fee_with_inviter(fee_with_inviter: Nat) -> Result<(), String> {
    STATS.with(|s| {
        let mut stats = s.borrow_mut();
        stats.fee_with_inviter = fee_with_inviter;
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

#[update(name = "setClosed", guard = "_is_auth")]
#[candid_method(update, rename = "setClosed")]
fn set_closed(closed: bool) -> Result<(), String> {
    STATS.with(|s| {
        let mut stats = s.borrow_mut();
        stats.closed = closed;
        Ok(())
    })
}

#[update(name = "setBaseExpire", guard = "_is_auth")]
#[candid_method(update, rename = "setBaseExpire")]
fn set_base_expire(base_expire: u64) -> Result<(), String> {
    STATS.with(|s| {
        let mut stats = s.borrow_mut();
        stats.base_expire = base_expire;
        Ok(())
    })
}

#[update(name = "setExtendExpire", guard = "_is_auth")]
#[candid_method(update, rename = "setExtendExpire")]
fn set_extend_expire(extend_expire: u64) -> Result<(), String> {
    STATS.with(|s| {
        let mut stats = s.borrow_mut();
        stats.extend_expire = extend_expire;
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

#[query(name = "getCapacityInfo")]
#[candid_method(query, rename = "getCapacityInfo")]
fn get_capacity_info(account: Principal) -> Result<CapacityInfo, String> {
    CAPACITY_INFO.with(|s| {
        let capacity_info = s.borrow();
        let info = capacity_info.get(&account);
        match info {
            Some(i) => Ok(i.clone()),
            None => Err("account not found".to_string()),
        }
    })
}

#[query(name = "getAllCapacityInfo")]
#[candid_method(query, rename = "getAllCapacityInfo")]
fn get_all_capacity_info(start: usize, limit: usize) -> Vec<CapacityInfo> {
    let mut count = 0;
    CAPACITY_INFO.with(|s| {
        let mut infos = Vec::new();
        for (_, p) in s.borrow().iter() {
            if count < start {
                count += 1;
                continue;
            }
            if infos.len() >= limit {
                break;
            }
            infos.push(p.clone());
            count += 1;
        }
        infos
    })
}

#[update(name = "activeCapacity", guard = "_is_closed")]
#[candid_method(update, rename = "activeCapacity")]
async fn active_capacity(
    block_index: Option<BlockIndex>,
    inviter: Option<Principal>,
) -> Result<(), String> {
    let caller = ic_cdk::caller();

    let (ledger, fee, fee_with_inviter, fee_to) = STATS.with(|s| {
        let stats = s.borrow();
        (
            stats.ledger.clone(),
            stats.fee.clone(),
            stats.fee_with_inviter.clone(),
            stats.fee_to.clone(),
        )
    });

    check_inviter(inviter)?;

    let charge_amount = if let Some(block_index) = block_index {
        check_charge_block(ledger, block_index, caller, fee_to).await?
    } else {
        0
    };

    let res = CAPACITY_INFO.with(|s| {
        let mut capacity_info = s.borrow_mut();
        if !capacity_info.contains_key(&caller) {
            capacity_info.insert(caller, CapacityInfo::default());
        }
        let info = capacity_info.get_mut(&caller);
        let mut info = match info {
            Some(i) => i,
            None => return Err("account not found".to_string()),
        };

        if charge_amount > 0 {
            save_charge_block(block_index)?;

            info.charged += charge_amount;
            info.rest_charged += charge_amount;
        }

        let fee = if info.long_term_capacity == 0 && inviter.is_some() {
            fee_with_inviter
        } else {
            fee
        };

        let mut new_account = false;
        if info.rest_charged >= fee {
            if info.long_term_capacity == 0 {
                new_account = true;
                if let Some(inviter) = inviter {
                    info.inviter = inviter;
                }
            }

            info.long_term_capacity += 1;
            info.rest_charged -= fee;
        }

        Ok(new_account)
    });

    match res {
        Ok(new_account) => {
            if new_account {
                if let Some(inviter) = inviter {
                    reward_inviter(inviter)?;
                }
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

#[update(name = "clearExpireCapacity")]
#[candid_method(update, rename = "clearExpireCapacity")]
fn clear_expire_capacity() -> Result<(), String> {
    CAPACITY_INFO.with(|s| {
        let mut capacity_info = s.borrow_mut();

        for (_, info) in capacity_info.iter_mut() {
            if info.reward_capacity != 0 && time() >= info.invitation_expire {
                info.invitee_count = 0;
                info.invitation_expire = 0;
                info.reward_capacity = 0;
            }
        }

        Ok(())
    })
}

#[pre_upgrade]
fn pre_upgrade() {
    let stats = STATS.with(|s| s.borrow().clone());
    let capacity_info = CAPACITY_INFO.with(|a| a.borrow().clone());
    let charge_block = CHARGE_BLOCK.with(|a| a.borrow().clone());
    ic_cdk::storage::stable_save((stats, capacity_info, charge_block)).unwrap();
}

#[post_upgrade]
fn post_upgrade() {
    let (stats_stored, capacity_store, charge_block) = ic_cdk::storage::stable_restore().unwrap();
    STATS.with(|s| {
        let mut stats = s.borrow_mut();
        *stats = stats_stored;
    });
    CAPACITY_INFO.with(|a| {
        let mut store = a.borrow_mut();
        *store = capacity_store;
    });
    CHARGE_BLOCK.with(|a| {
        let mut store = a.borrow_mut();
        *store = charge_block;
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
    fee_to: Principal,
) -> Result<u64, String> {
    let block_actived = CHARGE_BLOCK.with(|a| {
        let active_block = a.borrow();
        active_block.contains(&block_index)
    });
    if block_actived {
        return Err("block actived".to_string());
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
    let fee_to_id = AccountIdentifier::new(&fee_to, &DEFAULT_SUBACCOUNT);
    if to != fee_to_id {
        return Err("invalid transaction".to_string());
    }
    Ok(amount.e8s())
}

fn check_inviter(inviter: Option<Principal>) -> Result<(), String> {
    CAPACITY_INFO.with(|s| {
        let capacity_info = s.borrow();
        match inviter {
            Some(i) => {
                let info = capacity_info.get(&i);
                match info {
                    Some(info) => {
                        if info.long_term_capacity == 0 {
                            Err("invalid inviter".to_string())
                        } else {
                            Ok(())
                        }
                    }
                    None => Err("inviter not found".to_string()),
                }
            }
            None => Ok(()),
        }
    })
}

fn reward_inviter(inviter: Principal) -> Result<(), String> {
    let (base_expire, extend_expire) = STATS.with(|s| {
        let stats = s.borrow();
        (stats.base_expire, stats.extend_expire)
    });

    CAPACITY_INFO.with(|s| {
        let mut capacity_info = s.borrow_mut();
        let info = capacity_info.get_mut(&inviter);
        let mut info = match info {
            Some(i) => i,
            None => return Err("inviter not found".to_string()),
        };

        info.invitee_count += 1;

        if INVITE_REWARD.len() == 0 {
            return Ok(());
        }

        let (first_invitee_count, first_reward) = INVITE_REWARD[0];
        let (last_invitee_count, _) = INVITE_REWARD[INVITE_REWARD.len() - 1];

        if info.invitee_count == first_invitee_count {
            // first invite
            info.invitation_expire = time() + base_expire * 1000000000;
            info.reward_capacity = first_reward;
        } else if info.invitee_count > last_invitee_count {
            // mission complete
            info.invitation_expire += extend_expire * 1000000000;
        } else {
            for (count, reward) in INVITE_REWARD {
                if info.invitee_count == count {
                    info.reward_capacity += reward;
                    break;
                }
            }
        }

        Ok(())
    })
}

fn save_charge_block(block_index: Option<BlockIndex>) -> Result<(), String> {
    if let Some(block_index) = block_index {
        CHARGE_BLOCK.with(|a| {
            let mut block = a.borrow_mut();
            if block.insert(block_index) {
                Ok(())
            } else {
                Err("block_index already used".to_string())
            }
        })
    } else {
        Err("internal error".to_string())
    }
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
