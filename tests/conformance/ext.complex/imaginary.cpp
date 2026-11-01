// GNU imaginary constants (clang 19 `NumericLiteralParser` and
// `ActOnNumericConstant`): an `i`/`j` suffix makes a `_Complex T` whose
// real part is zero; a standard ud-suffix (`i`, `if`, `il`) without a
// matching literal operator falls back to the constant; the
// -Wgnu-imaginary-constant extension diagnostic (WP-211).
//@ expect: compile-ok
//@ args: -Wgnu-imaginary-constant
//@ strict-diags
//@ diag: 20:20 warning "imaginary constants are a GNU extension"
//@ diag: 23:20 warning "imaginary constants are a GNU extension"
//@ diag: 26:34 warning "imaginary constants are a GNU extension"
//@ diag: 27:34 warning "imaginary constants are a GNU extension"
//@ diag: 28:34 warning "imaginary constants are a GNU extension"
//@ diag: 29:34 warning "imaginary constants are a GNU extension"
//@ diag: 30:37 warning "imaginary constants are a GNU extension"
//@ diag: 30:44 warning "imaginary constants are a GNU extension"
//@ diag: 32:66 warning "imaginary constants are a GNU extension"
//@ diag: 36:13 warning "user-defined literal suffixes not starting with '_' are reserved"
//@ diag: 38:34 warning "imaginary constants are a GNU extension"
constexpr auto a = 2.0i;
static_assert(__is_same(decltype(a), const _Complex double));
static_assert(__real__ a == 0 && __imag__ a == 2.0);
constexpr auto b = 3i + 1;
static_assert(__is_same(decltype(b), const _Complex int));
static_assert(__real__ b == 1 && __imag__ b == 3);
static_assert(__is_same(decltype(1.5fi), _Complex float));
static_assert(__is_same(decltype(1il), _Complex long));
static_assert(__is_same(decltype(7ui), _Complex unsigned));
static_assert(__is_same(decltype(1.0Lj), _Complex long double));
constexpr _Complex double c = 1.0 + 2.0i * 2.0i;
static_assert(__real__ c == -3.0 && __imag__ c == 0);
template <class T> constexpr _Complex double f(T x) { return x * 2.0i + T(1); }
static_assert(__real__ f(3.0) == 1.0 && __imag__ f(3.0) == 6.0);
// A literal operator for the suffix wins over the GNU constant.
struct I { int v; };
constexpr I operator""i(unsigned long long v) { return {int(v)}; }
static_assert(__is_same(decltype(4i), I) && (4i).v == 4);
static_assert(__is_same(decltype(4.0i), _Complex double));
