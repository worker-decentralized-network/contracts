export const idlFactory = ({ IDL }) => {
  const Result = IDL.Variant({ 'Ok' : IDL.Null, 'Err' : IDL.Text });
  const CapacityInfo = IDL.Record({
    'reward_capacity' : IDL.Nat64,
    'rest_charged' : IDL.Nat,
    'long_term_capacity' : IDL.Nat64,
    'inviter' : IDL.Principal,
    'charged' : IDL.Nat,
    'invitation_expire' : IDL.Nat64,
    'invitee_count' : IDL.Nat64,
  });
  const Result_1 = IDL.Variant({ 'Ok' : CapacityInfo, 'Err' : IDL.Text });
  const StatsData = IDL.Record({
    'fee' : IDL.Nat,
    'closed' : IDL.Bool,
    'fee_to' : IDL.Principal,
    'owner' : IDL.Principal,
    'fee_with_inviter' : IDL.Nat,
    'ledger' : IDL.Principal,
    'base_expire' : IDL.Nat64,
    'extend_expire' : IDL.Nat64,
  });
  const Result_2 = IDL.Variant({ 'Ok' : StatsData, 'Err' : IDL.Text });
  return IDL.Service({
    'activeCapacity' : IDL.Func(
        [IDL.Opt(IDL.Nat64), IDL.Opt(IDL.Principal)],
        [Result],
        [],
      ),
    'clearExpireCapacity' : IDL.Func([], [Result], []),
    'getAllCapacityInfo' : IDL.Func(
        [IDL.Nat64, IDL.Nat64],
        [IDL.Vec(CapacityInfo)],
        ['query'],
      ),
    'getCapacityInfo' : IDL.Func([IDL.Principal], [Result_1], ['query']),
    'getStats' : IDL.Func([], [Result_2], ['query']),
    'setBaseExpire' : IDL.Func([IDL.Nat64], [Result], []),
    'setClosed' : IDL.Func([IDL.Bool], [Result], []),
    'setExtendExpire' : IDL.Func([IDL.Nat64], [Result], []),
    'setFee' : IDL.Func([IDL.Nat], [Result], []),
    'setFeeTo' : IDL.Func([IDL.Principal], [Result], []),
    'setFeeWithInviter' : IDL.Func([IDL.Nat], [Result], []),
  });
};
export const init = ({ IDL }) => {
  return [IDL.Principal, IDL.Nat, IDL.Nat, IDL.Principal, IDL.Nat64, IDL.Nat64];
};
