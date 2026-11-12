#ifndef DECL_FORMS_H
#define DECL_FORMS_H
namespace df {
template <class T> struct B { B(int) {} B(T, T) {} int v = 1; void f() {} };
template <class T> struct D : B<T> { using B<T>::B; using B<T>::f; };
template <class T> struct E : D<T> { using D<T>::D; using D<T>::v; };

template <class T> struct Opt {
  union { char null_; T val_; };
  bool engaged_;
  constexpr Opt() : null_(), engaged_(false) {}
  constexpr Opt(T t) : val_(t), engaged_(true) {}
  constexpr T value_or(T d) const { return engaged_ ? val_ : d; }
};
struct U { union { char c; int i; }; struct { short a, b; }; U() : i(5) {} };

template <class C> struct os {
  class sentry;
  struct in { int f(); static int s; };
  int v = 3;
};
template <class C> class os<C>::sentry {
  bool ok;
public:
  explicit sentry(os& o);
  explicit operator bool() const { return ok; }
};
template <class C> os<C>::sentry::sentry(os& o) : ok(o.v == 3) {}
template <class C> int os<C>::in::f() { return 4; }
template <class C> int os<C>::in::s = 5;

struct S {
  int v = 4;
  int get(this const S& s) { return s.v; }
  template <class Self> auto&& ref(this Self&& self) { return static_cast<Self&&>(self).v; }
};

struct te {
  enum class id : int { a = 1, b, c };
  using enum id;
  int f() const { return int(b); }
};

template <class T> constexpr bool imp = sizeof(T) > 1;
template <class A, class Z> struct pr {
  A a; Z z;
  constexpr pr() : a(), z() {}
  template <class U, class V> explicit(!imp<U>) constexpr pr(U u, V v) : a(u), z(v) {}
  explicit(sizeof(A) == 1) constexpr pr(A x) : a(x), z() {}
};
inline pr<bool, long> need() { pr<bool, long> p(true); p.z = 3; return p; }

template <class T> constexpr auto twice(auto x) { return T(x) * 2; }
constexpr unsigned long long operator""_k(unsigned long long n) { return n * 1000; }

struct fb {
  struct rb { int k = 1; };
  template <class R> struct res : rb { R r{}; };
};
template <class R> struct fb::res<R&> : fb::rb { int g() { return 2; } };
} // namespace df
#endif
