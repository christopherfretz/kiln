// [meta.reflection.member.queries] (P2996R13), spec/reflection-members.md §2:
// a class of 2,000 members queried 10,000 times. The member list is built once
// (MemberCache) and each call copies a filtered prefix into its result, so the
// run allocates one result vector per call and nothing else on the constexpr
// heap: allocs counts exactly the 10,000 results, and the steps stay flat in
// the member count (the copy is one Oven step, not one per member).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oven: 41 allocs<=10000 steps<=110000 peak_bytes<=16384 calls<=45001
//@ meta-header: skip libstdc++'s constant-evaluated _Destroy and basic_string loops cost steps per element (follow-up), past the step limit here
#include "kiln_meta.h"
using namespace std::meta;

#define F1(p) int p##0; int p##1; int p##2; int p##3; int p##4; int p##5; int p##6; int p##7; int p##8; int p##9;
#define F10(p) F1(p##0) F1(p##1) F1(p##2) F1(p##3) F1(p##4) F1(p##5) F1(p##6) F1(p##7) F1(p##8) F1(p##9)
#define F100(p) \
  F10(p##0) F10(p##1) F10(p##2) F10(p##3) F10(p##4) F10(p##5) F10(p##6) F10(p##7) F10(p##8) F10(p##9)

struct Big {
  F10(a0) F10(a1) F10(a2) F10(a3) F10(a4) F10(a5) F10(a6) F10(a7) F10(a8) F10(a9)
 private:
  F10(b0) F10(b1) F10(b2) F10(b3) F10(b4) F10(b5) F10(b6) F10(b7) F10(b8) F10(b9)
};

consteval unsigned long queries() {
  unsigned long n = 0;
  for (int i = 0; i < 2500; ++i) {
    // Every member: 2,000 data members and the six implicit specials.
    n += members_of(^^Big, access_context::unchecked()).size();
    // The public half and the specials, from the global scope.
    n += members_of(^^Big, access_context::current()).size();
    n += nonstatic_data_members_of(^^Big, access_context::unprivileged()).size();
    n += nonstatic_data_members_of(^^Big, access_context::unchecked()).size();
  }
  return n;
}
static_assert(sizeof(Big) == 8000);
// The query loop's evaluation (the oven: line).
static_assert(queries() == 2500 * (2006 + 1006 + 1000 + 2000));

int main() {}
