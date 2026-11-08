// GCC __builtin_clear_padding(p) zeroes the padding bits of *p and keeps
// every value bit: the gaps between and after members, unnamed
// bit-fields and the bits a named bit-field does not declare, the byte
// of an empty class, the tail of a base class whose tail padding the
// derived class does not reuse, and in a union the bits that are
// padding in every member. bool, pointers and member pointers keep all
// their bytes; x87 long double keeps its 10 value bytes. Each object is
// filled with 0xff first, so a 00 byte below is a cleared padding byte.
// GCC 16.1 compiles and runs the same file; its output must agree.
//@ expect: run
//@ std: c++20
//@ skip(clang): clang has no __builtin_clear_padding (LLVM issue 64830)
//@ oracle: gcc
//@ strict-diags
//@ stdout-file: run-clear-padding-bytes.stdout
#include <cfloat>
#include <cstdio>
#include <cstring>

template <class T> void show(const char *name, T &obj) {
  std::memset(static_cast<void *>(&obj), 0xff, sizeof obj);
  __builtin_clear_padding(&obj);
  const unsigned char *p = reinterpret_cast<const unsigned char *>(&obj);
  std::printf("%s:", name);
  for (unsigned i = 0; i < sizeof obj; ++i)
    std::printf(" %02x", p[i]);
  std::printf("\n");
}

struct S1 { char c; int i; char d; };
struct Nest { char a; S1 s; short t; };
struct Arr { S1 a[2]; char t; };
struct Bo { bool b; int i; };
struct Ptrs { char c; void *p; int Bo::*mp; };
struct BF { unsigned a : 3; unsigned : 0; unsigned char b : 2; int c : 5; };
struct Bw { unsigned x : 3, y : 7; unsigned short z : 1; };
struct Unn { char a; int : 5; char b; };
union U { char c; short s; struct { char a; int b; } t; };
union U2 { int i; char c; };
struct Emp {};
struct E2 { Emp e; int i; char c; };
struct EB : Emp { char c; int i; };
struct NUA { [[no_unique_address]] Emp e; char c; int i; };
struct Pod { int i; char c; };
struct DPod : Pod { char d; };
struct NonPod {
  int get() const { return i + c; }
private:
  int i;
  char c;
};
struct DNonPod : NonPod { char d; };
struct alignas(16) Al { char c; };
struct __attribute__((packed)) Pk { char c; int i; };
struct I128 { char c; __int128 x; };

int main() {
  S1 s1; show("S1", s1);
  Nest n; show("Nest", n);
  Arr ar; show("Arr", ar);
  Bo bo; show("Bo", bo);
  Ptrs ps; show("Ptrs", ps);
  BF bf; show("BF", bf);
  Bw bw; show("Bw", bw);
  Unn un; show("Unn", un);
  U u; show("U", u);
  U2 u2; show("U2", u2);
  Emp e; show("Emp", e);
  E2 e2; show("E2", e2);
  EB eb; show("EB", eb);
  NUA nua; show("NUA", nua);
  DPod dp; show("DPod", dp);
  DNonPod dn; show("DNonPod", dn);
  Al al; show("Al", al);
  Pk pk; show("Pk", pk);
  I128 i1; show("I128", i1);
  S1 a3[3]; show("S1[3]", a3);
  Bw a22[2][2]; show("Bw[2][2]", a22);
  volatile S1 vs;
  std::memset(const_cast<S1 *>(&vs), 0xff, sizeof vs);
  __builtin_clear_padding(&vs);
  std::printf("volatile: %d %d\n", static_cast<unsigned char>(vs.c),
              static_cast<unsigned char>(vs.d));

  // A large array is cleared by a loop: 600 of its 1200 bytes are padding.
  static S1 big[100];
  std::memset(big, 0xff, sizeof big);
  __builtin_clear_padding(&big);
  unsigned zeros = 0, sum = 0;
  const unsigned char *bp = reinterpret_cast<const unsigned char *>(big);
  for (unsigned i = 0; i < sizeof big; ++i) {
    zeros += bp[i] == 0;
    sum += bp[i] == 0 ? i : 0;
  }
  std::printf("big: %u %u\n", zeros, sum);

  // long double: x87's 80-bit format pads 16 bytes to 10, IEEE quad and
  // double have no padding.
  struct LD { long double v; };
  LD ld;
  std::memset(&ld, 0xff, sizeof ld);
  __builtin_clear_padding(&ld);
  const unsigned char *lp = reinterpret_cast<const unsigned char *>(&ld);
  unsigned lz = 0;
  for (unsigned i = 0; i < sizeof ld; ++i)
    lz += lp[i] == 0;
  unsigned expect = LDBL_MANT_DIG == 64 ? sizeof(long double) - 10 : 0;
  std::printf("long double: %s\n", lz == expect ? "ok" : "bad");
}
