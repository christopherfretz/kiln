//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:53 error "expression not permitted as operand of fold expression"
//@ diag: 8:58 error "operators in fold expression must be the same"
//@ diag: 9:54 error "pack expansion does not contain any unexpanded parameter packs"
// [expr.prim.fold]/1: fold operands are cast-expressions; both operators must match
template <class... Ts> int f(Ts... ts) { return (ts + 1 + ...); }
template <class... Ts> int g(Ts... ts) { return (1 + ... * ts); }
template <class... Ts> int h(Ts... ts) { return (1 + ... + 2); }
