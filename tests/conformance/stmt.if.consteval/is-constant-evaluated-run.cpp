// [meta.const.eval]/1, [expr.const]/21: `std::is_constant_evaluated()`
// and `if consteval` agree. A trial fold (a plain `if` condition, an
// operand of `&&`, a `?:`, a non-constant initializer) yields false and the
// else branch; a constant initializer, a constexpr variable or a
// static_assert yields true (WP-588).
//@ expect: run
//@ stdout: 2 2 20 0 0
//@ stdout: 1 1 10 10
namespace std {
constexpr bool is_constant_evaluated() noexcept { return __builtin_is_constant_evaluated(); }
} // namespace std
extern "C" int printf(const char *, ...);

constexpr bool ice() { if consteval { return true; } else { return false; } }
constexpr bool sice() { return std::is_constant_evaluated(); }
inline int f() { if (sice()) return 1; return 2; }
int g(int x) { if (sice() && x) return 1; return 2; }
constexpr int h() { return sice() ? 10 : 20; }
constexpr bool same() { return sice() == ice(); }

static_assert(h() == 10 && sice() && same());
bool gi = sice();  // constant-initialized: manifestly constant-evaluated
constexpr int ch = h();

int main() {
  int y = sice();
  bool z = sice() != ice();
  printf("%d %d %d %d %d\n", f(), g(1), h(), y, z);
  printf("%d %d %d %d\n", gi, same(), ch, static_cast<int>(sizeof(int[h()]) / sizeof(int)));
}
