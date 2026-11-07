// [class.member.lookup], [support.types.layout]/1: the member designated by
// offsetof is found by class member lookup, so a data member of a
// (non-virtual) base class is designated too; its offset is the base
// subobject's offset plus the member's (clang BuildBuiltinOffsetOf; an
// options table of a derived struct naming its base's fields).
//@ args: -Wno-invalid-offsetof
//@ expect: run
//@ stdout: 16 24 28 0 8
#include <cstddef>
extern "C" int printf(const char*, ...);

struct B { int a; long capacity; };
struct E { char e; };
struct D : E, B { int x; union { int u; }; };
struct DD : D {};

static_assert(offsetof(D, capacity) == 16);
static_assert(offsetof(struct D, x) == 24);
static_assert(offsetof(D, u) == 28);
static_assert(offsetof(D, e) == 0);
static_assert(offsetof(DD, capacity) == offsetof(D, capacity));

template<class T> constexpr std::size_t off_cap() { return offsetof(T, capacity); }

int main() {
  D d{};
  d.capacity = 7;
  long* p = reinterpret_cast<long*>(reinterpret_cast<char*>(&d) + offsetof(D, capacity));
  if (*p != 7) return 1;
  printf("%zu %zu %zu %zu %zu\n", offsetof(D, capacity), offsetof(D, x), offsetof(DD, u),
         offsetof(DD, e), off_cap<B>());
}
