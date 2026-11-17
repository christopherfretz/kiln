// GCC's __builtin_shuffle in constant expressions: a constant when its
// operands are (GCC folds the permutation of constants), the mask lanes
// reduced modulo N or 2N, a constexpr function with a mask computed at
// compile time, and dependent operands in a template. Compared with
// GCC 16.1 (clang has no __builtin_shuffle). WP-1548.
//@ std: c++20
//@ expect: compile-ok
//@ requires: clang>=22
//@ oracle: gcc
//@ skip(clang): clang 19/22 have no __builtin_shuffle
//@ strict-diags
typedef int v4si __attribute__((vector_size(16)));
typedef unsigned char v16qi __attribute__((vector_size(16)));
typedef float v4sf __attribute__((vector_size(16)));

constexpr v4si a = {1, 2, 3, 4}, b = {5, 6, 7, 8};
constexpr v4si r2 = __builtin_shuffle(a, b, (v4si){7, 2, 13, -1});
static_assert(r2[0] == 8 && r2[1] == 3 && r2[2] == 6 && r2[3] == 8);
constexpr v4si r1 = __builtin_shuffle(a, (v4si){5, -1, 2, 0});
static_assert(r1[0] == 2 && r1[1] == 4 && r1[2] == 3 && r1[3] == 1);

constexpr v4sf f = {0.5f, 1.5f, 2.5f, 3.5f};
static_assert(__builtin_shuffle(f, (v4si){3, 2, 1, 0})[0] == 3.5f);

constexpr v16qi bytes = {0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15};
// A rotation by `k` lanes: the mask is computed, lanes past 15 wrap.
constexpr v16qi rotate(v16qi x, unsigned char k) { return __builtin_shuffle(x, bytes + k); }
static_assert(rotate(bytes, 3)[0] == 3 && rotate(bytes, 3)[13] == 0 && rotate(bytes, 19)[15] == 2);

template <class V, class M> constexpr V shuf(V x, V y, M m) { return __builtin_shuffle(x, y, m); }
static_assert(shuf(a, b, (v4si){4, 0, 5, 1})[2] == 6);
