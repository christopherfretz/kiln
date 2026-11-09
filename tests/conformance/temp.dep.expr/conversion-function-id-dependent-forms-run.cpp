// [temp.dep.expr]/3: a conversion-function-id naming a dependent type is
// type-dependent in every form that names it: as a member access
// (`x.operator T()`, `p->operator T()`, `this->operator T()`), as an
// unqualified id (an implicit `this->`, [class.mfct.non.static]/2), and
// qualified (`Base<T>::operator T()`, `D::operator T()`); and in
// `&C::operator T` ([expr.unary.op]/4). Lookup happens at instantiation
// ([temp.res.general]/9), so a conversion declared later in the class
// (`early`) or in a dependent base is found, and the canonical
// `Arr<T, 2>` / `V_t<T, 2>` names substitute their non-dependent
// argument `2`, and a canonical `decltype(T() + 0L)` its expression
// (WP-1126; clang's TreeTransform::TransformDeclarationNameInfo).
//@ expect: run
//@ std: c++20
//@ stdout: 7 7 7 7 7 5 5 5 5 5
//@ stdout: 3 6 4 6 4 6
//@ stdout: 4 4 4 4
#include <cstdio>
template <class T, int N> struct V { using type = T[N]; };
template <class T, int N> using V_t = typename V<T, N>::type;
template <class T, int N> struct Arr { T a[N]; };

template <class T> struct Base {
  T buf[2] = {T(4), T(6)};
  operator T() const { return T(7); }
  operator V_t<T, 2> *() { return &buf; }
  operator Arr<T, 2>() const { return {{buf[0], buf[1]}}; }
};
template <class T> struct D : Base<T> {
  T a() const { return this->operator T(); }
  T b() const { return Base<T>::operator T(); }
  T c(const D &x) const { return x.operator T(); }
  T d(const D *p) const { return p->operator T(); }
  T e() const { return D::operator T(); }
  // Named before D's own conversion is declared (complete-class context).
  T early() const { return operator Arr<T, 3>().a[2]; }
  T q() const { return Base<T>::operator Arr<T, 2>().a[1]; }
  T qp() { return (*Base<T>::operator V_t<T, 2> *())[0]; }
  T m(D &x) { return (*x.operator V_t<T, 2> *())[1]; }
  T ar(const D *p) const { return p->operator Arr<T, 2>().a[0]; }
  operator Arr<T, 3>() const { return {{1, 2, 3}}; }
};
template <class T> T addr(const D<T> &d) {
  Arr<T, 3> (D<T>::*pm)() const = &D<T>::operator Arr<T, 3>;
  Arr<T, 2> (Base<T>::*pb)() const = &Base<T>::operator Arr<T, 2>;
  return (d.*pm)().a[1] + (d.*pb)().a[0];
}

template <class T> struct S {
  T v;
  operator T() const { return v; }
  T u() const { return operator T(); }
  T q() const { return S::operator T(); }
  T r() const { return S<T>::operator T(); }
};
template <class T> T viaptr(const S<T> &s) {
  T (S<T>::*pm)() const = &S<T>::operator T;
  return (s.*pm)();
}
template <class T> T free_call(const S<T> &s) { return s.operator T(); }

template <class T> struct Dt {
  T v;
  operator decltype(T() + 0L)() const { return v + 1; }
  long f() const { return this->operator decltype(T() + 0L)(); }
  long g() const { return operator decltype(T() + 0L)(); }
  long h() const { return Dt::operator decltype(T() + 0L)(); }
};
template <class T> long dt(const Dt<T> &x) { return x.operator decltype(T() + 0L)(); }

int main() {
  D<int> d;
  S<long> s{5};
  std::printf("%d %d %d %d %d %ld %ld %ld %ld %ld\n", d.a(), d.b(), d.c(d), d.d(&d), d.e(),
              s.u(), s.q(), s.r(), viaptr(s), free_call(s));
  std::printf("%d %d %d %d %d %d\n", d.early(), d.q(), d.qp(), d.m(d), d.ar(&d), addr(d));
  Dt<int> t{3};
  std::printf("%ld %ld %ld %ld\n", t.f(), t.g(), t.h(), dt(t));
}
