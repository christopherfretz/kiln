// __has_builtin answers 1 for every carry builtin (clang LanguageExtensions
// "Multiprecision Arithmetic Builtins"; kiln answers it only because it
// declares, evaluates and lowers them), and __has_constexpr_builtin too
// (clang 19 marks them Constexpr). Verified against clang 19.
//@ expect: compile-ok
#define BOTH(x) (__has_builtin(x) && __has_constexpr_builtin(x))
static_assert(BOTH(__builtin_addcb) && BOTH(__builtin_addcs) && BOTH(__builtin_addc) &&
              BOTH(__builtin_addcl) && BOTH(__builtin_addcll));
static_assert(BOTH(__builtin_subcb) && BOTH(__builtin_subcs) && BOTH(__builtin_subc) &&
              BOTH(__builtin_subcl) && BOTH(__builtin_subcll));
#if !__has_builtin(__builtin_addcll)
#error "__builtin_addcll"
#endif
// The probe libraries use: a use guarded by it compiles.
#if __has_builtin(__builtin_addcll)
unsigned long long add2(unsigned long long a, unsigned long long b, unsigned long long *c) {
  return __builtin_addcll(a, b, 0, c);
}
#endif
static_assert(!__has_builtin(__builtin_addcq));
int main() {}
