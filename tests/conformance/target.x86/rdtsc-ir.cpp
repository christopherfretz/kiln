// [driver] `__rdtsc` is clang's plain x86-64 target builtin
// (`BuiltinsX86.def` `BUILTIN(__rdtsc, "UOi", "")`: `unsigned long long ()`,
// not noexcept), the same lowering as `__builtin_ia32_rdtsc`
// (`call i64 @llvm.x86.rdtsc()`). <ia32intrin.h> defines `_rdtsc()` as a
// macro for it, with no declaration, so a use is an implicit builtin
// (boringssl's jitter entropy source, x86_64 sweep). WP-961.
//@ requires: target-x86_64
//@ expect: ir
static_assert(__has_builtin(__rdtsc));
static_assert(!noexcept(__rdtsc()));
static_assert(__is_same(decltype(__rdtsc()), unsigned long long));
unsigned long long ticks() { return __rdtsc(); }
unsigned long long elapsed() {
  unsigned long long t = __rdtsc();
  return __rdtsc() - t;
}
