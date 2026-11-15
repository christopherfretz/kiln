// [over.match.class.deduct]/1.1: the guide formed from a constructor has
// the constructor's parameter types, and names in them are checked for
// access as the constructor's declaration is, in its class
// ([class.access.general]/7), not at namespace scope: a protected member
// type of a base, a private one of the class itself, one befriended by
// another class, through a constructor template and an alias template.
// Found running GCC's C++ testsuite.
//@ expect: compile-ok
template <class T> struct Base {
protected:
  using type = T;
};
template <class T> struct C1 : Base<T> {
  C1(T, typename Base<T>::type);
};
C1 c1(1, 1);
static_assert(__is_same(decltype(c1), C1<int>));

template <class T> struct C2 {
private:
  using t = T;

public:
  C2(T, t);
};
C2 c2(1, 2);

struct NB {
protected:
  struct type {};
};
template <class T> struct D : NB {
  D(T, typename T::type);
};
D d = {NB(), {}};

template <class> struct Cont;
template <class T> class Fr {
  using type = T;
  friend Cont<T>;
};
template <class T> struct Cont {
  using argument_type = typename Fr<T>::type;
  Cont(T, argument_type);
};
Cont cont(1, 1);

template <class T> struct C4 {
private:
  using v = T;

public:
  template <class U> C4(T, U, v);
};
C4 c4(1, 2.0, 3);
static_assert(__is_same(decltype(c4), C4<int>));

template <class T> using A1 = C2<T>;
A1 a1(1, 2);
static_assert(__is_same(decltype(a1), C2<int>));
template <class T> using A2 = C1<T>;
A2 a2(1, 2);

// A user-declared guide is checked in its own scope; clang 22 and GCC 16.1
// accept this one too (the private name is reached through G<T>, whose
// access the deduction does not check before it succeeds).
template <class T> struct G {
private:
  using t = T;

public:
  G(T, int);
};
template <class T> G(T, typename G<T>::t) -> G<T>;
G g(1, 2);

template <class T> void in_tmpl(T x) {
  C1 c(x, x);
  static_assert(__is_same(decltype(c), C1<T>));
}
template void in_tmpl(long);
