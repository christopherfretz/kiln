// [expr.sub]/2, [expr.add]/4, [expr.shift]/1, [expr.ass]/7, [conv.integral]:
// a wide `_BitInt(N)` (N > 128) operand of an operator whose result is
// narrow (a subscript's index, a pointer offset, a shift count of a narrow
// left operand, the right operand of such a compound assignment, a
// conversion to a narrow type) is evaluated by value, left operand first
// (spec/oven.md "`_BitInt`"). clang is the oracle; x86-64 only (aarch64's
// `__BITINT_MAXWIDTH__` is 128).
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: compile-ok

using S = _BitInt(200);
using U = unsigned _BitInt(200);
constexpr S big = S(1) << 150;
constexpr int arr[4] = {10, 20, 30, 40};

// Subscripts, either way round.
static_assert(arr[S(1)] == 20);
static_assert(U(3)[arr] == 40);
static_assert(arr[big >> 149] == 30);

// Pointer offsets.
static_assert(*(arr + S(2)) == 30);
static_assert(*(S(1) + arr) == 20);
static_assert(*(arr + 4 - U(1)) == 40);
static_assert(arr + S(4) == arr + 4);

// Shifts of a narrow left operand by a wide count: the result has the left
// operand's promoted type.
static_assert((1 << S(3)) == 8);
static_assert((-64 >> U(2)) == -16);
static_assert(((char)1 << S(9)) == 512);
static_assert((1ULL << (big >> 145)) == 1ULL << 32);
static_assert((1 << S(3) << 2) == 32);

constexpr int compound() {
  const int *p = arr;
  p += S(3);
  p -= U(2);
  int x = *p;      // 20
  x <<= S(2);      // 80
  x >>= U(1);      // 40
  x += big >> 148; // 44
  x *= S(2);       // 88
  x %= S(50);      // 38
  x ^= S(1);       // 39
  x += big;        // computed wide, converted back: 39
  return x;
}
static_assert(compound() == 39);

struct B { int f : 7; };
constexpr int bit_field() {
  B b{3};
  b.f <<= S(4); // 48
  b.f >>= S(1); // 24
  return b.f;
}
static_assert(bit_field() == 24);

// Conversions to narrow types.
enum E { e0, e1, e2 };
static_assert((E)S(2) == e2);
static_assert((bool)big && !(bool)S(0));
static_assert((unsigned char)(big + 300) == 44);
static_assert((double)big == 0x1p150);
