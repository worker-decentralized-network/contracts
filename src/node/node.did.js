export const idlFactory = ({ IDL }) => {
  const StatsData = IDL.Record({
    'closed' : IDL.Bool,
    'token' : IDL.Principal,
    'owner' : IDL.Principal,
  });
  const Result = IDL.Variant({ 'Ok' : StatsData, 'Err' : IDL.Text });
  const Result_1 = IDL.Variant({ 'Ok' : IDL.Null, 'Err' : IDL.Text });
  const Result_2 = IDL.Variant({ 'Ok' : IDL.Nat, 'Err' : IDL.Text });
  return IDL.Service({
    'getBackers' : IDL.Func([], [IDL.Vec(IDL.Principal)], ['query']),
    'getKeepers' : IDL.Func([], [IDL.Vec(IDL.Principal)], ['query']),
    'getStats' : IDL.Func([], [Result], ['query']),
    'setClosed' : IDL.Func([IDL.Bool], [Result_1], []),
    'withdraw' : IDL.Func([IDL.Principal, IDL.Nat], [Result_2], []),
  });
};
export const init = ({ IDL }) => { return [IDL.Principal, IDL.Principal]; };
