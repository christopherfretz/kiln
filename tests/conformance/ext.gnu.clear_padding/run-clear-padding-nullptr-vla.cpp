// GCC __builtin_clear_padding(p): a std::nullptr_t subobject of a class
// (a member, a member of a member or base, a union member) holds no value
// ([conv.lval]/3.1 never reads it), so its bytes are padding there; a
// std::nullptr_t object that is not inside a class, or an array of them,
// is left alone, as GCC does. A pointer to a variable length array (GNU)
// clears the padding of every element, the count known at run time. Each
// object is filled with 0xff first, so a 00 byte is a cleared padding byte.
// Found running GCC's C++ testsuite (kiln cleared neither). GCC 16.1
// compiles and runs the same file; its output must agree. WP-1694.
//@ expect: run
//@ std: c++20
//@ skip(clang): clang has no __builtin_clear_padding (LLVM issue 64830)
//@ oracle: gcc
//@ strict-diags
//@ args: -Wno-vla-cxx-extension
//@ stdout: np:ffffffffffffffff np2:ffffffffffffffffffffffffffffffff A:0000000000000000 B:ff000000000000000000 U:ffff000000000000 E:0000000000000000ff00 vla:ff00ffffff00ff00ffffff00 vla2:24 vla3:24 vla0:ok
#include <cstdio>
#include <cstring>

typedef decltype(nullptr) np_t;
typedef np_t np2[2];
struct A { np_t n; };
struct B { char c; np_t n; short s; };
union U { np_t n; short i; };
struct E : A { char c; };
struct T { char a; short b; char c; };

void dump(const char *name, const void *p, unsigned long n, unsigned long max) {
  std::printf("%s:", name);
  for (unsigned long i = 0; i < n && i < max; i++)
    std::printf("%02x", static_cast<const unsigned char *>(p)[i]);
  std::printf(" ");
}
#define SHOW(name, ty, max)                                                    \
  {                                                                            \
    ty x;                                                                      \
    std::memset(&x, 0xff, sizeof x);                                           \
    __builtin_clear_padding(&x);                                               \
    dump(name, &x, sizeof x, max);                                             \
  }

// The count of elements whose bytes match one T cleared by GCC's rule.
int cleared(const T *p, int n) {
  static const unsigned char want[6] = {0xff, 0, 0xff, 0xff, 0xff, 0};
  int k = 0;
  for (int i = 0; i < n; i++)
    k += std::memcmp(p + i, want, sizeof want) == 0;
  return k;
}

__attribute__((noinline)) void vla(int m, int n) {
  T a[m];
  std::memset(&a, 0xff, sizeof a);
  __builtin_clear_padding(&a);
  dump("vla", &a, sizeof a, 12);
  T b[m][n];
  std::memset(&b, 0xff, sizeof b);
  __builtin_clear_padding(&b);
  std::printf("vla2:%d ", cleared(&b[0][0], m * n));
  T c[m][n][2];
  std::memset(&c, 0xff, sizeof c);
  __builtin_clear_padding(&c);
  std::printf("vla3:%d ", cleared(&c[0][0][0], m * n * 2) / 2);
}

__attribute__((noinline)) void vla0(int m) {
  T guard[2];
  std::memset(&guard, 0xff, sizeof guard);
  T (*p)[m] = nullptr;
  if (m == 0) {
    // A zero count clears nothing (and touches nothing).
    auto q = reinterpret_cast<T (*)[m]>(&guard);
    __builtin_clear_padding(q);
  }
  (void)p;
  std::printf("vla0:%s\n", guard[0].b == -1 && guard[1].c == char(-1) ? "ok" : "bad");
}

int main() {
  SHOW("np", np_t, 64)
  SHOW("np2", np2, 64)
  SHOW("A", A, 64)
  SHOW("B", B, 10)
  SHOW("U", U, 64)
  SHOW("E", E, 10)
  vla(2, 12);
  vla0(0);
}
