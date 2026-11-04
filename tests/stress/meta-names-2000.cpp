// [meta.reflection.names] (P2996R13), spec/reflection.md §3.9: 2,000
// named classes and their members, 16 names-family queries each. The
// strings are frozen once per (reflection, kind) and the results built in
// place, so the run allocates nothing on the constexpr heap and the
// static space holds each name once.
// display_string_of of a member is qualified ("A0123::m"), as GCC prints it.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oven: 56 allocs<=0 steps<=150000 calls<=40000 static_bytes<=196608 frozen_bytes<=300000
#include "kiln_meta.h"
using namespace std::meta;

#define D1(p)                                                                                    \
  struct p##0 { int m; }; struct p##1 { int m; }; struct p##2 { int m; }; struct p##3 { int m; }; \
  struct p##4 { int m; }; struct p##5 { int m; }; struct p##6 { int m; }; struct p##7 { int m; }; \
  struct p##8 { int m; }; struct p##9 { int m; };
#define D10(p) D1(p##0) D1(p##1) D1(p##2) D1(p##3) D1(p##4) D1(p##5) D1(p##6) D1(p##7) D1(p##8) D1(p##9)
#define D100(p) \
  D10(p##0) D10(p##1) D10(p##2) D10(p##3) D10(p##4) D10(p##5) D10(p##6) D10(p##7) D10(p##8) D10(p##9)
D100(A0) D100(A1)

#define R1(p) ^^p##0, ^^p##1, ^^p##2, ^^p##3, ^^p##4, ^^p##5, ^^p##6, ^^p##7, ^^p##8, ^^p##9,
#define R10(p) R1(p##0) R1(p##1) R1(p##2) R1(p##3) R1(p##4) R1(p##5) R1(p##6) R1(p##7) R1(p##8) R1(p##9)
#define R100(p) \
  R10(p##0) R10(p##1) R10(p##2) R10(p##3) R10(p##4) R10(p##5) R10(p##6) R10(p##7) R10(p##8) R10(p##9)
#define M1(p) \
  ^^p##0::m, ^^p##1::m, ^^p##2::m, ^^p##3::m, ^^p##4::m, ^^p##5::m, ^^p##6::m, ^^p##7::m, ^^p##8::m, ^^p##9::m,
#define M10(p) M1(p##0) M1(p##1) M1(p##2) M1(p##3) M1(p##4) M1(p##5) M1(p##6) M1(p##7) M1(p##8) M1(p##9)
#define M100(p) \
  M10(p##0) M10(p##1) M10(p##2) M10(p##3) M10(p##4) M10(p##5) M10(p##6) M10(p##7) M10(p##8) M10(p##9)

constexpr info classes[] = {R100(A0) R100(A1)};
constexpr info members[] = {M100(A0) M100(A1)};
static_assert(sizeof(classes) / sizeof(info) == 2000);

consteval unsigned long queries() {
  unsigned long n = 0;
  for (const info c : classes) {
    n += has_identifier(c) + identifier_of(c).size() + u8identifier_of(c).size();
    n += display_string_of(c).size() + u8display_string_of(c).size();
    n += source_location_of(c).line() > 0;
    n += identifier_of(c).data() == identifier_of(c).data();
    n += identifier_of(c)[0] == 'A';
  }
  for (const info m : members) {
    n += has_identifier(m) + identifier_of(m).size() + u8identifier_of(m).size();
    n += display_string_of(m).size() + u8display_string_of(m).size();
    n += source_location_of(m).line() > 0;
    n += identifier_of(m) == "m";
    n += u8identifier_of(m) == u8"m";
  }
  return n;
}
static_assert(queries() == 2000 * (1 + 5 + 5 + 5 + 5 + 3) + 2000 * (1 + 1 + 1 + 8 + 8 + 3));

int main() {}
