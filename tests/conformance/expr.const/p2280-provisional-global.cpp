// [expr.const]/4, /10 (C++23, P2280R4): an `extern` variable named before
// its definition is an unknown object for as long as it has no
// initializer; once the definition (later in the translation unit) gives
// it one, it is usable in constant expressions. kiln resolves such a
// variable provisionally and reuses that object while it is still
// undefined (spec/oven.md "Unknown references"; WP-378): identity holds
// across evaluations, and the definition still takes effect.
//@ expect: compile-ok
//@ strict-diags
//@ std: c++23
//@ requires: clang>=22
extern const int &r;
extern const int n;
extern const int a[3];
constexpr bool same(const int &x, const int &y) { return &x == &y; }
constexpr int read(const int &x) { return x; }

static_assert(same(r, r));
static_assert(same(n, n));
static_assert(&a[1] - a == 1);
static_assert(same(r, r)); // again: the same provisional objects
static_assert(same(n, n));
static_assert(&a[2] - &a[0] == 2);

const int v = 3;
const int &r = v;
const int n = 4;

static_assert(same(r, v));
static_assert(read(r) == 3);
static_assert(read(n) == 4);
static_assert(same(n, n));
