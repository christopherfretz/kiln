//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:50 error "expression contains unexpanded parameter pack 'ts'"
//@ diag: 15:44 error "expression contains unexpanded parameter pack 'ts'"
//@ diag: 16:52 error "initializer contains unexpanded parameter pack 'ts'"
//@ diag: 17:50 error "expression contains unexpanded parameter pack 'ts'"
//@ diag: 18:48 error "expression contains unexpanded parameter pack 'ts'"
//@ diag: 19:49 error "initializer contains unexpanded parameter pack 'ts'"
//@ diag: 20:51 error "expression contains unexpanded parameter pack 'ts'"
//@ diag: 21:49 error "initializer contains unexpanded parameter pack 'ts'"
// [temp.variadic]/12: a pack named outside a pack expansion is ill-formed.
// It is diagnosed in the template definition, once per full-expression or
// initializer (at a braced or parenthesized initializer's opening bracket).
template <class... Ts> int g1(Ts... ts) { return ts; }
template <class... Ts> void g2(Ts... ts) { ts; }
template <class... Ts> void g3(Ts... ts) { int k = ts + 1; }
template <class... Ts> int g4(Ts... ts) { return sizeof(ts); }
template <class... Ts> void g5(Ts... ts) { if (ts) {} }
template <class... Ts> void g6(Ts... ts) { int k{ts}; }
template <class... Ts> void g7(Ts... ts) { while (ts > 0) {} }
template <class... Ts> void g8(Ts... ts) { int k(1, ts); }
// Expansions, and lambdas that capture the pack inside an expansion or a
// pack index, are fine.
template <class... Ts> int ok1(Ts... ts) { return (ts + ...); }
template <class... Ts> int ok2(Ts... ts) { return [&] { return (ts + ...); }(); }
template <class... Ts> int ok3(Ts... ts) { return [&] { return ts...[0]; }(); }
int a = ok1(1, 2) + ok2(1, 2) + ok3(1, 2);
