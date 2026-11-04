// Argument checks of __builtin_prefetch (clang SemaChecking
// `BuiltinPrefetch`, `void (const void *, ...)`): at most three arguments;
// `addr` converts to `const void *`; `rw` and `locality` are integer
// constant expressions in [0, 1] and [0, 3]; the result is void. Same at
// clang 19 and 22 (WP-610).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:3 error "argument value 2 is outside the valid range [0, 1]"
//@ diag: 17:3 error "argument value 4 is outside the valid range [0, 3]"
//@ diag: 18:3 error "argument value -1 is outside the valid range [0, 1]"
//@ diag: 19:3 error "argument to '__builtin_prefetch' must be a constant integer"
//@ diag: 20:3 error "argument to '__builtin_prefetch' must be a constant integer"
//@ diag: 21:3 error "argument to '__builtin_prefetch' must be a constant integer"
//@ diag: 22:32 error "too many arguments to function call, expected at most 3, have 4"
void f(const void *p, int n) {
  __builtin_prefetch(p, 2);
  __builtin_prefetch(p, 0, 4);
  __builtin_prefetch(p, -1);
  __builtin_prefetch(p, n);
  __builtin_prefetch(p, 0, n);
  __builtin_prefetch(p, 1.0);
  __builtin_prefetch(p, 0, 3, 1);
}
enum Hint { Read, Write };
constexpr int kLocality = 2;
void ok(const void *p, int *q) {
  __builtin_prefetch(p);
  __builtin_prefetch(q, Write);
  __builtin_prefetch(q + 1, true, kLocality);
  __builtin_prefetch(nullptr, Read, 0);
  static_assert(__is_same(decltype(__builtin_prefetch(p)), void));
}
