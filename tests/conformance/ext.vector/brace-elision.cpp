// Brace elision of vector members ([dcl.init.aggr]/16; clang 19
// CheckSubElementType): a GNU vector member cannot be initialized from a
// scalar, so the scalar starts an elided sub-list of up to N elements; a
// vector initializer copy-initializes the member; an ext vector member
// splats the scalar instead of eliding (WP-173).
//@ expect: run
typedef int g4i __attribute__((vector_size(16)));
typedef float g2f __attribute__((vector_size(8)));
typedef int v4i __attribute__((ext_vector_type(4)));
struct I4 { int x[4]; };
constexpr bool eq(g4i v, int a, int b, int c, int d) {
  I4 s = __builtin_bit_cast(I4, v);
  return s.x[0] == a && s.x[1] == b && s.x[2] == c && s.x[3] == d;
}
struct S { g4i a; int b; };
constexpr S s = {1, 2, 3, 4, 5};
static_assert(eq(s.a, 1, 2, 3, 4) && s.b == 5);
constexpr S s2 = {1, 2};
static_assert(eq(s2.a, 1, 2, 0, 0) && s2.b == 0);
constexpr S s3 = {{7}, 8};
static_assert(eq(s3.a, 7, 0, 0, 0) && s3.b == 8);
constexpr S s4 = {s.a, 9};
static_assert(eq(s4.a, 1, 2, 3, 4) && s4.b == 9);
constexpr g4i arr[2] = {1, 2, 3, 4, 5};
static_assert(eq(arr[0], 1, 2, 3, 4) && eq(arr[1], 5, 0, 0, 0));
struct N { int x; S s; int y; };
constexpr N n = {1, 2, 3, 4, 5, 6, 7};
static_assert(n.x == 1 && eq(n.s.a, 2, 3, 4, 5) && n.s.b == 6 && n.y == 7);
struct F { g2f f; int z; };
constexpr F f = {1.5f, 2.5f, 3};
struct F2 { float x[2]; };
static_assert(__builtin_bit_cast(F2, f.f).x[0] == 1.5f && __builtin_bit_cast(F2, f.f).x[1] == 2.5f);
static_assert(f.z == 3);
// An ext vector member splats a scalar: q = {7, 7, 7, 7}, z = 8.
struct E { v4i q; int z; };
constexpr E e = {7, 8};
static_assert(e.z == 8);
static_assert(eq((g4i)e.q, 7, 7, 7, 7));
S rt = {1, 2, 3, 4, 5};
g4i rarr[2] = {1, 2, 3, 4, 5, 6};
int main() { return rt.b + __builtin_bit_cast(I4, rarr[1]).x[1] - 11; }
