// A builtin function's name may only be called directly (clang
// `err_builtin_fn_use`); a call through parentheses is direct.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "builtin functions must be directly called"
//@ diag: 10:11 error "builtin functions must be directly called"
//@ diag: 11:20 warning "'__builtin_is_constant_evaluated' will always evaluate to 'true' in a manifestly constant-evaluated expression"
//@ diag: 12:12 error "builtin functions must be directly called"
bool (*q)() = __builtin_is_constant_evaluated;
auto p = &__builtin_is_constant_evaluated;
constexpr bool b = (__builtin_is_constant_evaluated)();
void h() { __builtin_is_constant_evaluated; }
