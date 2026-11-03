// Clang ext vectors, bool element type, as union members ([class.union]
// p2, [expr.const]; clang VectorExprEvaluator): initializing, assigning
// and nesting a `bool` ext vector member of a union is a constant
// expression; the lane stores go through the union like a bit-field's
// (spec/oven.md "Bool vectors"). WP-438.
//@ expect: run
typedef bool b8 __attribute__((ext_vector_type(8)));
union U { b8 v; unsigned char c; };
struct S { int i; U u; };

constexpr U u = {(b8)true};
static_assert(__builtin_bit_cast(unsigned char, u.v) == 255);
constexpr S s = {1, {(b8){true, false, true}}};
static_assert(__builtin_bit_cast(unsigned char, s.u.v) == 5);
constexpr unsigned char f() {
  U w{.c = 3};
  w.v = (b8){false, true};
  return __builtin_bit_cast(unsigned char, w.v);
}
static_assert(f() == 2);
constexpr U arr[2] = {{(b8)true}, {(b8){true}}};
static_assert(__builtin_bit_cast(unsigned char, arr[0].v) == 255);
static_assert(__builtin_bit_cast(unsigned char, arr[1].v) == 1);

int main() {
  if (__builtin_bit_cast(unsigned char, u.v) != 255) return 1;
  if (__builtin_bit_cast(unsigned char, s.u.v) != 5) return 2;
  if (f() != 2) return 3;
  if (__builtin_bit_cast(unsigned char, arr[1].v) != 1) return 4;
  return 0;
}
