// [bit.cast]/2: `Dst::low` of the result takes padding bits of the source
// and holds an indeterminate value; the determinate bit-fields of its
// byte read their values, an assignment makes it determinate, a bit-cast
// copies the determinate bits, and an `unsigned char` may hold the
// indeterminate byte unread. Indeterminate bits of objects deallocated
// within the evaluation hold no memory after it.
//@ expect: compile-ok
//@ std: c++20
//@ oracle: gcc
//@ xfail(clang): clang 22 does not bit-cast bit-fields in constant evaluation
struct Dst { unsigned char low : 5, high : 3; };
struct Src { unsigned char low : 1, : 4, high : 3; };
constexpr Dst mk() { return __builtin_bit_cast(Dst, Src{1, 2}); }

constexpr int assigned() { Dst d = mk(); d.low = 3; return d.low + d.high; }
static_assert(assigned() == 5);
constexpr int neighbour() { return mk().high; }
static_assert(neighbour() == 2);
constexpr int recast() { Dst d = mk(); return __builtin_bit_cast(Dst, d).high; }
static_assert(recast() == 2);
struct B { unsigned char c; };
constexpr int byte() { Dst d = mk(); B b = __builtin_bit_cast(B, d); return 1; }
static_assert(byte() == 1);
constexpr int copied() { Dst d = mk(); d.low = 1; Dst e = d; return e.low + e.high; }
static_assert(copied() == 3);

struct Sx { unsigned short a : 4, : 8, b : 4; };
struct Dx { unsigned char a : 4, x : 4; unsigned char y : 4, b : 4; };
constexpr int span() { Dx d = __builtin_bit_cast(Dx, Sx{1, 2}); return d.a + d.b; }
static_assert(span() == 3);

constexpr int heap() {
  int s = 0;
  for (int i = 0; i < 1000; ++i) {
    Dst *p = new Dst(mk());
    s += p->high;
    delete p;
  }
  return s;
}
static_assert(heap() == 2000);
