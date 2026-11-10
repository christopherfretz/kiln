// [meta.const.eval]/3-4 (P2641R4), [class.union]/2, [basic.life]/1:
// `__builtin_is_within_lifetime(p)` (libc++'s `std::is_within_lifetime`) is
// a consteval builtin that is true when `p` points to an object within its
// lifetime: an active union member, not an inactive one (through the
// pointer's designator, anonymous unions and variant members included),
// a variable of this or an earlier evaluation, a live heap object, through
// any pointer conversion. A conforming `std::` wrapper sees the member its
// argument designates. clang 20 added the builtin.
//@ requires: clang>=22
//@ expect: compile-ok
//@ strict-diags
//@ xfail(gcc): GCC 16.1 has no __builtin_is_within_lifetime
namespace std {
template <class T> consteval bool is_within_lifetime(const T* p) noexcept {
  return __builtin_is_within_lifetime(p);
}
} // namespace std

static_assert(__is_same(decltype(__builtin_is_within_lifetime((int*)0)), bool));
static_assert(noexcept(__builtin_is_within_lifetime((void*)0)));

constexpr int g = 0;
static_assert(__builtin_is_within_lifetime(&g));
static_assert(__builtin_is_within_lifetime(static_cast<const void*>(&g)));
static_assert(std::is_within_lifetime(&g) && std::is_within_lifetime<const void>(&g));

constexpr union { int m1; int m2; } cu{.m2 = 1};
static_assert(!__builtin_is_within_lifetime(&cu.m1) && __builtin_is_within_lifetime(&cu.m2));
static_assert(!std::is_within_lifetime(&cu.m1) && std::is_within_lifetime(&cu.m2));

struct S { int a; };
union U { int a; int b; S s; };
struct V { union { int m1; int m2; }; int n; };

consteval bool locals() {
  int i = 0;
  const void* vp = &i;
  bool ok = __builtin_is_within_lifetime(&i) && __builtin_is_within_lifetime(vp);
  U u;
  ok = ok && __builtin_is_within_lifetime(&u) && !__builtin_is_within_lifetime(&u.a);
  u.b = 1;
  ok = ok && !__builtin_is_within_lifetime(&u.a) && __builtin_is_within_lifetime(&u.b);
  u.s = S{2};
  ok = ok && __builtin_is_within_lifetime(&u.s.a) && !std::is_within_lifetime(&u.b);
  ok = ok && std::is_within_lifetime(&(u.s).a) && std::is_within_lifetime((const void*)&u.s);
  union { int m1; int m2; };
  ok = ok && !__builtin_is_within_lifetime(&m1) && !std::is_within_lifetime(&m2);
  m1 = 1;
  ok = ok && __builtin_is_within_lifetime(&m1) && !std::is_within_lifetime(&m2);
  V v;
  ok = ok && !__builtin_is_within_lifetime(&v.m1) && __builtin_is_within_lifetime(&v.n);
  v.m2 = 1;
  ok = ok && !std::is_within_lifetime(&v.m1) && std::is_within_lifetime(&v.m2);
  S s; // default-initialized: within its lifetime
  return ok && __builtin_is_within_lifetime(&s);
}
static_assert(locals());

struct W {
  union { int x; long y; };
  consteval bool live_x() const { return __builtin_is_within_lifetime(&this->x); }
  consteval bool live_y() const { return std::is_within_lifetime(&y); }
};
consteval bool members() {
  W w;
  w.y = 2;
  return !w.live_x() && w.live_y();
}
static_assert(members());

consteval bool heap() {
  int* p = new int(1);
  bool ok = __builtin_is_within_lifetime(p) && std::is_within_lifetime(p);
  delete p;
  int* a = new int[3]{};
  ok = ok && __builtin_is_within_lifetime(a + 2);
  delete[] a;
  return ok;
}
static_assert(heap());

// A constexpr function template calling it escalates to an immediate
// function ([expr.const]/20), so its address cannot be taken outside an
// immediate function context and a SFINAE default argument rejects it.
template <class T> constexpr void escalates(T p) { std::is_within_lifetime(p); }
template <class T, void (*)(T) = &escalates<T>> constexpr bool check(int) { return false; }
template <class T> constexpr bool check(long) { return true; }
static_assert(check<int*>(0) && check<void*>(0));
