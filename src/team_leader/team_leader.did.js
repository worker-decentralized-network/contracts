export const idlFactory = ({ IDL }) => {
  const ProfileStatus = IDL.Variant({
    'Passed' : IDL.Null,
    'Refused' : IDL.Null,
    'Pending' : IDL.Null,
  });
  const Profile = IDL.Record({
    'status' : ProfileStatus,
    'wallet_address' : IDL.Principal,
    'comment' : IDL.Text,
    'discord_account' : IDL.Text,
    'community_name' : IDL.Text,
    'number' : IDL.Nat16,
  });
  const Result = IDL.Variant({ 'Ok' : Profile, 'Err' : IDL.Text });
  const StatsData = IDL.Record({
    'fee' : IDL.Nat,
    'closed' : IDL.Bool,
    'admin' : IDL.Principal,
    'fee_to' : IDL.Principal,
    'owner' : IDL.Principal,
    'ledger' : IDL.Principal,
  });
  const Result_1 = IDL.Variant({ 'Ok' : StatsData, 'Err' : IDL.Text });
  const Result_2 = IDL.Variant({ 'Ok' : IDL.Null, 'Err' : IDL.Text });
  return IDL.Service({
    'get' : IDL.Func([], [Result], ['query']),
    'getAll' : IDL.Func([IDL.Nat64, IDL.Nat64], [IDL.Vec(Profile)], ['query']),
    'getStats' : IDL.Func([], [Result_1], ['query']),
    'insert' : IDL.Func([IDL.Nat64, IDL.Text, IDL.Text], [Result_2], []),
    'pass' : IDL.Func([IDL.Principal], [Result_2], []),
    'refuse' : IDL.Func([IDL.Principal, IDL.Text], [Result_2], []),
    'setAdmin' : IDL.Func([IDL.Principal], [Result_2], []),
    'setClosed' : IDL.Func([IDL.Bool], [Result_2], []),
    'setFee' : IDL.Func([IDL.Nat], [Result_2], []),
    'setFeeTo' : IDL.Func([IDL.Principal], [Result_2], []),
    'setLedger' : IDL.Func([IDL.Principal], [Result_2], []),
    'updateProfile' : IDL.Func([Profile], [Result_2], []),
  });
};
export const init = ({ IDL }) => { return []; };
