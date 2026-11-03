// [c.math.syn], [cpp.cond], [expr.const]: roundeven (C23 7.12.9.8),
// canonicalize (C23 7.12.11.7) and exp10 (C23 7.12.6.2) are builtins at
// every -fclang-compat level, with clang's types (`noexcept`, as the
// `nothrow` attribute makes them), and canonicalize / exp10 are not
// constant expressions (clang 19 and 22 fold neither; roundeven is a P0533
// rounding builtin kiln folds, see constexpr-rounding.cpp). Codegen:
// tests/codegen/builtins_c_math.cpp (WP-410).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 47:15 error "static assertion expression is not an integral constant expression"
//@ diag: 48:15 error "static assertion expression is not an integral constant expression"
//@ diag: 49:15 error "static assertion expression is not an integral constant expression"
//@ diag: 50:15 error "static assertion expression is not an integral constant expression"
static_assert(__has_builtin(__builtin_roundeven) && __has_builtin(__builtin_roundevenf) &&
              __has_builtin(__builtin_roundevenl));
static_assert(__has_builtin(__builtin_canonicalize) && __has_builtin(__builtin_canonicalizef) &&
              __has_builtin(__builtin_canonicalizel));
static_assert(__has_builtin(__builtin_exp10) && __has_builtin(__builtin_exp10f) &&
              __has_builtin(__builtin_exp10l));
static_assert(!__has_constexpr_builtin(__builtin_canonicalize));
static_assert(!__has_constexpr_builtin(__builtin_exp10));

extern "C" {
double __builtin_roundeven(double) noexcept;
float __builtin_roundevenf(float) noexcept;
long double __builtin_roundevenl(long double) noexcept;
double __builtin_canonicalize(double) noexcept;
float __builtin_canonicalizef(float) noexcept;
long double __builtin_canonicalizel(long double) noexcept;
double __builtin_exp10(double) noexcept;
float __builtin_exp10f(float) noexcept;
long double __builtin_exp10l(long double) noexcept;
}

template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;

void f(double d, float x, long double l) {
  static_assert(same<decltype(__builtin_roundeven(d)), double>);
  static_assert(same<decltype(__builtin_roundevenf(x)), float>);
  static_assert(same<decltype(__builtin_canonicalizel(l)), long double>);
  static_assert(same<decltype(__builtin_exp10f(x)), float>);
  static_assert(same<decltype(__builtin_exp10l(l)), long double>);
  static_assert(noexcept(__builtin_canonicalize(d)) && noexcept(__builtin_exp10(d)));
}

static_assert(__builtin_canonicalize(2.5) == 2.5);
static_assert(__builtin_canonicalizef(2.5f) == 2.5f);
static_assert(__builtin_exp10(2.0) == 100.0);
static_assert(__builtin_exp10l(2.0L) == 100.0L);
