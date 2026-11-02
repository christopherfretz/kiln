// [over.match.best.general]/2.1, [over.match.ctor]/1, CWG2327 (clang's
// workaround, [dcl.init.general]/16.6.2.1): direct-initializing `Tup<U>`
// from a class derived from `Tup<T>` that also has a conversion function
// template to `Tup<V>` considers the constructors (a converting
// constructor template binding the base by derived-to-base conversion)
// and the conversion function template specialized for `Tup<U>`, whose
// implicit object argument is an exact match: the conversion function
// wins, also when it is explicit. A copy-initialization from the
// unrelated class ([over.match.copy]) has no explicit candidates. The
// shape of libc++ 19 tuple PR31384.pass.cpp; clang 19 returns 0.
//@ expect: run
int count = 0;
template <class From, class To>
constexpr bool convertible = __is_convertible(From, To);
template <class T>
struct Tup {
  T t{};
  Tup() = default;
  template <class U>
    requires __is_constructible(T, U)
  explicit(!convertible<U, T>) Tup(U u) : t(u) {}
  template <class U>
    requires __is_constructible(T, U&&)
  explicit(!convertible<U&&, T>) Tup(Tup<U>&& o) : t(static_cast<U&&>(o.t)) {}
};
struct Explicit {
  Explicit() = default;
  explicit Explicit(int) {}
};
struct Implicit {
  Implicit() = default;
  Implicit(int) {}
};
template <class T>
struct Derived : Tup<T> {
  using Tup<T>::Tup;
  template <class U>
  operator Tup<U>() && { ++count; return {}; }
};
template <class T>
struct ExplicitDerived : Tup<T> {
  using Tup<T>::Tup;
  template <class U>
  explicit operator Tup<U>() && { ++count; return {}; }
};
static_assert(!convertible<ExplicitDerived<int>, Tup<Explicit>>);
static_assert(convertible<ExplicitDerived<int>, Tup<Implicit>>);
int main() {
  int r = 0;
  Tup<Explicit> a = Derived<int>{42};
  r |= count != 1;
  Derived<int> d{42};
  Tup<Explicit> b(static_cast<Derived<int>&&>(d));
  r |= (count != 2) << 1;
  count = 0;
  Tup<Implicit> c = Derived<int>{42};
  Derived<int> d2{42};
  Tup<Implicit> e(static_cast<Derived<int>&&>(d2));
  r |= (count != 2) << 2;
  count = 0;
  ExplicitDerived<int> d3{42};
  Tup<Explicit> f(static_cast<ExplicitDerived<int>&&>(d3));
  r |= (count != 1) << 3;
  count = 0;
  Tup<Implicit> g = ExplicitDerived<int>{42};
  r |= (count != 0) << 4;
  ExplicitDerived<int> d4{42};
  Tup<Implicit> h(static_cast<ExplicitDerived<int>&&>(d4));
  r |= (count != 1) << 5;
  (void)a, (void)b, (void)c, (void)e, (void)f, (void)g, (void)h;
  return r;
}
