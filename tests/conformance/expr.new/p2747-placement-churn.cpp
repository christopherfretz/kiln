// [expr.new]/24, [expr.const]/5.18 (C++26, P2747R2) + kiln Oven budgets
// (spec/oven.md "Placement new", the WP-060 reclaim rule): 100000 objects
// constructed by placement new in a loop, each destroyed before the next,
// first in a union member of a local, then in one heap object's storage.
// The storage is reused, not leaked: one heap allocation in all, and the
// heap and arena peak at one object, not 100000.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
//@ args: -fconstexpr-steps=100000000
//@ oven: 40 allocs<=1 peak_bytes<=128 peak_arena<=256
//@ mem<=512
namespace std { using size_t = decltype(sizeof 0); }
void *operator new(std::size_t, void *p) noexcept;
struct Obj {
  long long v[4];
  int *live;
  constexpr Obj(int i, int *n) : v{i, i + 1, i + 2, i + 3}, live(n) { ++*live; }
  constexpr ~Obj() { --*live; }
};
union Slot { char none; Obj obj; constexpr Slot() : none() {} constexpr ~Slot() {} };
constexpr long long churn(int n) {
  int live = 0;
  long long sum = 0;
  Slot s;
  for (int i = 0; i < n; ++i) {
    Obj *p = ::new (&s.obj) Obj(i, &live);
    sum += p->v[3] - p->v[0];
    p->~Obj();
  }
  Obj *heap = static_cast<Obj *>(static_cast<void *>(new Obj(0, &live)));
  for (int i = 0; i < n; ++i) {
    heap->~Obj();
    ::new (heap) Obj(i, &live);
    sum += heap->v[1] - i;
  }
  delete heap;
  return live == 0 ? sum : -1;
}
static_assert(churn(100000) == 400000);
