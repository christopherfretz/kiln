//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:68 error "unary fold expression has empty expansion for operator '*' with no fallback value"
//@ diag: 7:9 note "in instantiation of function template specialization 'prod<>' requested here"
// [temp.variadic]/10: an empty unary fold of another operator is ill-formed.
template <class... Ts> constexpr int prod(Ts... ts) { return (ts * ...); }
int x = prod();
