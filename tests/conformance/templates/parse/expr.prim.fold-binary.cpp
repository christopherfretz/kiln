//@ expect: ast
//@ oracle: clang
// [expr.prim.fold]/1: binary folds with an init operand
template <int... Ns> int sum() { return (Ns + ... + 0); }
template <int... Ns> int prod() { return (1 * ... * Ns); }
template <class... Ts> int shifts(Ts... ts) { return (1 << ... << ts); }
