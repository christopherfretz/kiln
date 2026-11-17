// GCC's __builtin_shuffle on clang's ext vectors of three lanes (kiln
// accepts any vector type; GCC's own vectors always have a power of two
// lanes): the mask lanes, read as unsigned, are reduced modulo 3 (one
// operand) or 6 (two operands), with a variable mask, a constant one and
// in a constant expression. kiln-only (neither clang nor GCC has both
// the builtin and the type). WP-1548.
//@ std: c++20
//@ expect: run
//@ requires: clang>=22
//@ skip(clang): clang 19/22 have no __builtin_shuffle
//@ strict-diags
//@ stdout: 30 10 20 | 20 5 30 | 6 4 10 | 20 10 30
extern "C" int printf(const char *, ...);
typedef int v3 __attribute__((ext_vector_type(3)));
typedef unsigned u3 __attribute__((ext_vector_type(3)));

__attribute__((noinline)) v3 one(v3 a, u3 m) { return __builtin_shuffle(a, m); }
__attribute__((noinline)) v3 two(v3 a, v3 b, u3 m) { return __builtin_shuffle(a, b, m); }

constexpr v3 ca = {10, 20, 30};
constexpr v3 cr = __builtin_shuffle(ca, (u3){5, 7, 9});
static_assert(cr.x == 30 && cr.y == 20 && cr.z == 10);

int main() {
  v3 a = {10, 20, 30}, b = {4, 5, 6};
  v3 r = one(a, (u3){2, 3, 7});      // 2, 0, 1
  v3 s = two(a, b, (u3){1, 10, 8});  // a1, b1 (10 % 6 = 4), a2 (8 % 6 = 2)
  v3 t = two(a, b, (u3){5, 0xffffffffu, 6}); // b2, 2^32-1 % 6 = 3: b0, 0: a0
  v3 c = __builtin_shuffle(a, b, (u3){1, 6, 2});
  printf("%d %d %d | %d %d %d | %d %d %d | %d %d %d\n", r.x, r.y, r.z, s.x, s.y, s.z, t.x, t.y,
         t.z, c.x, c.y, c.z);
}
