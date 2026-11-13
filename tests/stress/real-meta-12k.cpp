// [meta.syn] through libstdc++ 16.1's real <meta> (WP-484): 12,500
// reflection calls over 100 data members, the names real binding code
// uses most (identifier_of, display_string_of as string_view; type_of,
// is_public, dealias, has_identifier), with the members as the header's
// std::vector<info> and access_context::unchecked() its own struct. GCC
// 16.1 compiles the same file. The budgets keep the per-call cost of the
// header's classes at that of kiln's own (meta-names-2000.cpp).
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
//@ strict-diags
//@ mem<=1024
//@ oven: 39 allocs<=1 steps<=70000 calls<=20000 peak_bytes<=16384 frozen_bytes<=8192 static_bytes<=24576
#include <meta>
#include <string_view>
using namespace std::meta;

#define F10(p) int p##0, p##1, p##2, p##3, p##4, p##5, p##6, p##7, p##8, p##9;
struct S {
  F10(a) F10(b) F10(c) F10(d) F10(e) F10(f) F10(g) F10(h) F10(i) F10(j)
};
using A = S;

consteval unsigned long queries() {
  unsigned long n = 0;
  auto ms = nonstatic_data_members_of(dealias(^^A), access_context::unchecked());
  for (int rep = 0; rep < 25; ++rep)
    for (info m : ms) {
      n += identifier_of(m).size();
      n += display_string_of(m).size();
      n += type_of(m) == ^^int;
      n += is_public(m);
      n += has_identifier(m);
    }
  return n;
}
// Per member and pass: 2 (the identifier), 5 ("S::a0"), 1, 1, 1.
static_assert(queries() == 25 * 100 * (2 + 5 + 1 + 1 + 1));
