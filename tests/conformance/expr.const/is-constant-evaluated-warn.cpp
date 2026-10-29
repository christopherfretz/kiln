// [expr.const]/20: `__builtin_is_constant_evaluated()` (or
// `std::is_constant_evaluated()`) called directly in a context that is
// always constant-evaluated is always true; clang warns
// (-Wconstant-evaluated, on by default).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 15:15 warning "'__builtin_is_constant_evaluated' will always evaluate to 'true' in a manifestly constant-evaluated expression"
//@ diag: 16:15 warning "'std::is_constant_evaluated' will always evaluate to 'true' in a manifestly constant-evaluated expression"
//@ diag: 17:21 warning "'__builtin_is_constant_evaluated' will always evaluate to 'true' in a manifestly constant-evaluated expression"
//@ diag: 19:17 warning "'__builtin_is_constant_evaluated' will always evaluate to 'true' in a manifestly constant-evaluated expression"
namespace std {
constexpr bool is_constant_evaluated() noexcept { return __builtin_is_constant_evaluated(); }
} // namespace std
using namespace std;
static_assert(__builtin_is_constant_evaluated());
static_assert(is_constant_evaluated());
constexpr bool b = !__builtin_is_constant_evaluated();
void f(int n) {
  if constexpr (__builtin_is_constant_evaluated()) {
  }
  if (__builtin_is_constant_evaluated() && n) {
  }
  int x = __builtin_is_constant_evaluated();
  (void)x;
}
