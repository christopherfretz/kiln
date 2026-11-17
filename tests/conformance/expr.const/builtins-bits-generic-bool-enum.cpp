// [expr.const]: clang's generic bit builtins __builtin_clzg, __builtin_ctzg
// and __builtin_popcountg take any type clang calls an unsigned integer
// type, `bool` and an unscoped enumeration with an unsigned underlying type
// included (clang 22 accepts them; GCC 16.1 rejects both: kiln follows
// clang for the builtins clang has). `bool` is one bit wide, an
// enumeration as wide as its underlying type. Found running GCC's C++
// testsuite. WP-1612.
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
enum Eu : unsigned { eu1 = 1 };
enum Eb : unsigned char { eb0 };

static_assert(__builtin_clzg(true) == 0);
static_assert(__builtin_ctzg(true) == 0);
static_assert(__builtin_popcountg(true) == 1);
static_assert(__builtin_popcountg(false) == 0);
static_assert(__builtin_clzg(false, 9) == 9);
static_assert(__builtin_ctzg(false, -1) == -1);
static_assert(__builtin_clzg(eu1) == 31);
static_assert(__builtin_ctzg(Eu(8)) == 3);
static_assert(__builtin_popcountg(Eb(0xff)) == 8);
static_assert(__builtin_clzg(eb0, true) == 1);

template <class T> constexpr int pop(T x) { return __builtin_popcountg(x); }
static_assert(pop(true) == 1 && pop(eu1) == 1);
