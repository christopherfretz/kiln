// [dcl.type.auto.deduct]/2.2, /4, [dcl.constexpr]/10: a constrained
// `decltype(auto)` placeholder is checked against `decltype(e)`; the
// `const` that `constexpr` adds to the variable's type is not part of the
// deduced type (libc++ is_always_lock_free.pass.cpp, lockfree.pass.cpp:
// `constexpr same_as<const bool> decltype(auto) r = X::is_always_lock_free;`).
//@ expect: compile-ok
//@ strict-diags
template <class T, class U> concept same = __is_same(T, U);
struct X { static constexpr bool v = true; };
constexpr same<const bool> decltype(auto) a = X::v;
constexpr same<int> decltype(auto) b = 1;
static_assert(__is_same(decltype(a), const bool));
static_assert(__is_same(decltype(b), const int));
// The variable keeps `constexpr`'s `const` unless `decltype(e)` is a reference.
constexpr decltype(auto) u = 1;
static_assert(__is_same(decltype(u), const int));
static int h;
constexpr decltype(auto) r = (h);
static_assert(__is_same(decltype(r), int &));
constexpr decltype(auto) cr = (X::v);
static_assert(__is_same(decltype(cr), const bool &));
struct M { static constexpr same<int> decltype(auto) m = 2; };
static_assert(__is_same(decltype(M::m), const int));
void f() {
  constexpr same<const bool> decltype(auto) c = X::v;
  same<bool> decltype(auto) d = !c;
  (void)d;
}
