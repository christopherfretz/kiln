// `__builtin_is_constant_evaluated` is declared on first use (clang
// `LazilyCreateBuiltin`): its name has the placeholder type
// `<builtin fn type>` and the call converts it with `BuiltinFnToFnPtr`.
//@ expect: ast
//@ oracle: clang
constexpr bool f() { return __builtin_is_constant_evaluated(); }
bool g() { return (__builtin_is_constant_evaluated)() || f(); }
