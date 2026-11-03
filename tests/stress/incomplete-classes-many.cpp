// [meta.reflection.define.aggregate] (P2996R13), spec/reflection.md §6.2:
// 5,000 forward-declared classes, each given an Oven type slot while it is
// incomplete (a pointer to it), then 10,000 more evaluations: 100 consteval
// blocks that each complete one class with define_aggregate, each followed
// by an evaluation that allocates one object of it, and 9,800 fillers.
// A completion refreshes only its own provisional slot (a completion epoch
// plus the set of provisional slots), so no evaluation rescans the 5,000.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oven: evaluations<=15100 allocs<=100 steps<=40000 bytes<=24000 peak_bytes<=256 static_bytes<=32
#include "kiln_meta.h"
using namespace std::meta;

template <class T> constexpr bool null_ptr() {
  T* p = nullptr;
  return p == nullptr;
}
consteval void complete(info r) {
  define_aggregate(r, {data_member_spec(^^int, {.name = "v"}), data_member_spec(^^int, {.name = "w"})});
}
template <class T> constexpr int use() {
  T* p = new T{1, 2};
  int v = p->v + p->w;
  delete p;
  return v;
}
constexpr int id(int i) { return i; }

#define X10(M, p) M(p##0) M(p##1) M(p##2) M(p##3) M(p##4) M(p##5) M(p##6) M(p##7) M(p##8) M(p##9)
#define X100(M, p) \
  X10(M, p##0) X10(M, p##1) X10(M, p##2) X10(M, p##3) X10(M, p##4) X10(M, p##5) X10(M, p##6) X10(M, p##7) X10(M, p##8) X10(M, p##9)
#define X1000(M, p)                                                                             \
  X100(M, p##0) X100(M, p##1) X100(M, p##2) X100(M, p##3) X100(M, p##4) X100(M, p##5) X100(M, p##6) \
  X100(M, p##7) X100(M, p##8) X100(M, p##9)
#define X5000(M) X1000(M, C0) X1000(M, C1) X1000(M, C2) X1000(M, C3) X1000(M, C4)

#define DECL(c) struct c;
#define SLOT(c) static_assert(null_ptr<c>());
X5000(DECL)
X5000(SLOT)

// 100 blocks, each completing one class (every 50th), then one use of it
// and 98 fillers: 100 * (1 + 1 + 98) = 10,000 evaluations.
#define F1 static_assert(id(__LINE__) > 0);
#define F7 F1 F1 F1 F1 F1 F1 F1
#define F98 F7 F7 F7 F7 F7 F7 F7 F7 F7 F7 F7 F7 F7 F7
#define COMPLETE(c) consteval { complete(^^c); } static_assert(use<c>() == 3); F98
COMPLETE(C0000)
COMPLETE(C0050)
COMPLETE(C0100)
COMPLETE(C0150)
COMPLETE(C0200)
COMPLETE(C0250)
COMPLETE(C0300)
COMPLETE(C0350)
COMPLETE(C0400)
COMPLETE(C0450)
COMPLETE(C0500)
COMPLETE(C0550)
COMPLETE(C0600)
COMPLETE(C0650)
COMPLETE(C0700)
COMPLETE(C0750)
COMPLETE(C0800)
COMPLETE(C0850)
COMPLETE(C0900)
COMPLETE(C0950)
COMPLETE(C1000)
COMPLETE(C1050)
COMPLETE(C1100)
COMPLETE(C1150)
COMPLETE(C1200)
COMPLETE(C1250)
COMPLETE(C1300)
COMPLETE(C1350)
COMPLETE(C1400)
COMPLETE(C1450)
COMPLETE(C1500)
COMPLETE(C1550)
COMPLETE(C1600)
COMPLETE(C1650)
COMPLETE(C1700)
COMPLETE(C1750)
COMPLETE(C1800)
COMPLETE(C1850)
COMPLETE(C1900)
COMPLETE(C1950)
COMPLETE(C2000)
COMPLETE(C2050)
COMPLETE(C2100)
COMPLETE(C2150)
COMPLETE(C2200)
COMPLETE(C2250)
COMPLETE(C2300)
COMPLETE(C2350)
COMPLETE(C2400)
COMPLETE(C2450)
COMPLETE(C2500)
COMPLETE(C2550)
COMPLETE(C2600)
COMPLETE(C2650)
COMPLETE(C2700)
COMPLETE(C2750)
COMPLETE(C2800)
COMPLETE(C2850)
COMPLETE(C2900)
COMPLETE(C2950)
COMPLETE(C3000)
COMPLETE(C3050)
COMPLETE(C3100)
COMPLETE(C3150)
COMPLETE(C3200)
COMPLETE(C3250)
COMPLETE(C3300)
COMPLETE(C3350)
COMPLETE(C3400)
COMPLETE(C3450)
COMPLETE(C3500)
COMPLETE(C3550)
COMPLETE(C3600)
COMPLETE(C3650)
COMPLETE(C3700)
COMPLETE(C3750)
COMPLETE(C3800)
COMPLETE(C3850)
COMPLETE(C3900)
COMPLETE(C3950)
COMPLETE(C4000)
COMPLETE(C4050)
COMPLETE(C4100)
COMPLETE(C4150)
COMPLETE(C4200)
COMPLETE(C4250)
COMPLETE(C4300)
COMPLETE(C4350)
COMPLETE(C4400)
COMPLETE(C4450)
COMPLETE(C4500)
COMPLETE(C4550)
COMPLETE(C4600)
COMPLETE(C4650)
COMPLETE(C4700)
COMPLETE(C4750)
COMPLETE(C4800)
COMPLETE(C4850)
COMPLETE(C4900)
COMPLETE(C4950)
static_assert(sizeof(C0000) == 2 * sizeof(int) && sizeof(C4950) == sizeof(C0000));
