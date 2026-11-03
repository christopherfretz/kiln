// [expr.const]/20, [basic.start.static]/2: an initializer of a variable
// with static or thread storage duration, or of a variable that might be
// usable in constant expressions (a const integral local, a reference),
// is first evaluated as a constant initializer, which is manifestly
// constant-evaluated: a direct `__builtin_is_constant_evaluated()` or
// `std::is_constant_evaluated()` call there warns (-Wconstant-evaluated)
// even when that evaluation then fails (`? x : 7` reads non-constant
// `x`) and the variable is initialized dynamically or by a re-fold
// (clang `Sema::CheckCompleteVariableDeclaration`). A non-const local
// (`l3`) is never constant-initialized and does not warn. The same
// wording at -fclang-compat=19 (spec/sema.md "Constant initialization").
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
//@ diag: 31:10 warning "'__builtin_is_constant_evaluated' will always evaluate to 'true' in a manifestly constant-evaluated expression"
//@ diag: 32:10 warning "'__builtin_is_constant_evaluated' will always evaluate to 'true' in a manifestly constant-evaluated expression"
//@ diag: 33:10 warning "'std::is_constant_evaluated' will always evaluate to 'true' in a manifestly constant-evaluated expression"
//@ diag: 34:23 warning "'__builtin_is_constant_evaluated' will always evaluate to 'true' in a manifestly constant-evaluated expression"
//@ diag: 39:12 warning "'__builtin_is_constant_evaluated' will always evaluate to 'true' in a manifestly constant-evaluated expression"
//@ diag: 40:18 warning "'__builtin_is_constant_evaluated' will always evaluate to 'true' in a manifestly constant-evaluated expression"
//@ diag: 42:19 warning "'__builtin_is_constant_evaluated' will always evaluate to 'true' in a manifestly constant-evaluated expression"
//@ diag: 43:18 warning "'__builtin_is_constant_evaluated' will always evaluate to 'true' in a manifestly constant-evaluated expression"
//@ diag: 44:18 warning "'__builtin_is_constant_evaluated' will always evaluate to 'true' in a manifestly constant-evaluated expression"
//@ diag: 49:10 warning "'__builtin_is_constant_evaluated' will always evaluate to 'true' in a manifestly constant-evaluated expression"
//@ diag: 50:17 warning "'__builtin_is_constant_evaluated' will always evaluate to 'true' in a manifestly constant-evaluated expression"
//@ diag: 51:27 warning "'__builtin_is_constant_evaluated' will always evaluate to 'true' in a manifestly constant-evaluated expression"
namespace std {
constexpr bool is_constant_evaluated() noexcept { return __builtin_is_constant_evaluated(); }
} // namespace std
int x = 3;
int g1 = __builtin_is_constant_evaluated() ? x : 7;
int g2 = __builtin_is_constant_evaluated() ? 1 : 7;
int g3 = std::is_constant_evaluated() ? x : 7;
thread_local int g4 = __builtin_is_constant_evaluated() ? x : 7;
struct S {
  static int m;
  static const int c;
};
int S::m = __builtin_is_constant_evaluated() ? x : 7;
const int S::c = __builtin_is_constant_evaluated() ? 2 : 7;
int f() {
  static int s1 = __builtin_is_constant_evaluated() ? x : 7;
  const int l1 = __builtin_is_constant_evaluated() ? 2 : 7;
  const int l2 = __builtin_is_constant_evaluated() ? x : 7;
  int l3 = __builtin_is_constant_evaluated() ? 2 : 7;
  const double l4 = __builtin_is_constant_evaluated() ? 2 : 7;
  return s1 + l1 + l2 + l3 + (int)l4;
}
int &r = __builtin_is_constant_evaluated() ? x : x;
const int &cr = __builtin_is_constant_evaluated() ? x : 7;
template <class T> T tv = __builtin_is_constant_evaluated() ? x : 7;
int use = tv<int>;
