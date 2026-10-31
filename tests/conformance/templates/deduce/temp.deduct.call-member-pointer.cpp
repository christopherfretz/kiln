// [temp.deduct.type]/8: T C::* deduces both T and C.
struct S { int a; long b; };
template <class T, class C> constexpr int sz(T C::*) { return sizeof(T); }
static_assert(sz(&S::a) == sizeof(int));
static_assert(sz(&S::b) == sizeof(long));
