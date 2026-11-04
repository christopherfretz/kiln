// [temp.inst]/8: what is named in a discarded statement ([stmt.if]/2) is
// not odr-used, but a specialization is still instantiated where its
// existence affects the semantics: a deduced return type is needed for
// the call's type ([dcl.spec.auto.general]/12), and a constexpr or
// consteval function, or a variable usable in constant expressions, may
// be needed for constant evaluation ([expr.const]/22; clang instantiates
// it at the reference, `isImplicitlyDefinableConstexprFunction`). From
// C++23 (P2448R2) a defaulted copy assignment is constexpr, so it is
// defined too, and calls `W<int>`'s; the copy constructor of a class with
// a virtual base is not constexpr, so it is not (and `W(const W&)` stays
// uninstantiated). A lambda's body is no part of the enclosing discarded
// statement, and a generic lambda's call operator has a deduced return
// type. Each static_assert here fires for that reason.
//@ expect: compile-fail
//@ std: c++23
//@ strict-diags
//@ diag: 36:17 error "static assertion failed due to requirement 'never<int>': deduced"
//@ diag: 74:11 note "in instantiation of function template specialization 'deduced<int>' requested here"
//@ diag: 40:17 error "static assertion failed due to requirement 'never<int>': constexpr"
//@ diag: 75:11 note "in instantiation of function template specialization 'cexpr<int>' requested here"
//@ diag: 44:17 error "static assertion failed due to requirement 'never<int>': consteval"
//@ diag: 76:11 note "in instantiation of function template specialization 'ceval<int>' requested here"
//@ diag: 49:19 error "static assertion failed due to requirement 'never<int>': constexpr member"
//@ diag: 77:13 note "in instantiation of member function 'A<int>::m' requested here"
//@ diag: 53:37 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 78:11 note "in instantiation of variable template specialization 'cvar<int>' requested here"
//@ diag: 62:19 error "static assertion failed due to requirement 'never<int>': W assign"
//@ diag: 67:8 note "in instantiation of member function 'W<int>::operator=' requested here"
//@ diag: 55:17 error "static assertion failed due to requirement 'never<int>': plain"
//@ diag: 81:23 note "in instantiation of function template specialization 'plain<int>' requested here"
//@ diag: 55:17 error "static assertion failed due to requirement 'never<double>': plain"
//@ diag: 82:31 note "in instantiation of function template specialization 'plain<double>' requested here"
template <class T> inline constexpr bool never = false;

template <class T> auto deduced(T) {
  static_assert(never<T>, "deduced");
  return 1;
}
template <class T> constexpr int cexpr(T) {
  static_assert(never<T>, "constexpr");
  return 1;
}
template <class T> consteval int ceval(T) {
  static_assert(never<T>, "consteval");
  return 1;
}
template <class T> struct A {
  constexpr int m() {
    static_assert(never<T>, "constexpr member");
    return 1;
  }
};
template <class T> const int cvar = T::not_a_member;
template <class T> int plain(T) {
  static_assert(never<T>, "plain");
  return 1;
}
template <class T> struct W {
  W() {}
  W(const W &) { static_assert(never<T>, "W copy"); }
  W &operator=(const W &) {
    static_assert(never<T>, "W assign");
    return *this;
  }
};
struct Base {};
struct B : virtual Base {
  W<int> w;
};

int use(B *p) {
  if constexpr (false) {
    A<int> a;
    (void)deduced(1);
    (void)cexpr(1);
    (void)ceval(1);
    (void)a.m();
    (void)cvar<int>;
    (void)B(*p);
    *p = *p;
    (void)[] { return plain(1); };
    (void)[](auto x) { return plain(x); }(1.0);
  }
  return 0;
}
