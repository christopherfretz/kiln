// [temp.dep.expr]/3, /5: a call naming members of the current
// instantiation (a class with no dependent base) is not type-dependent
// when overload resolution can be done at definition and cannot choose
// differently at instantiation: candidates whose types are not dependent
// (an overload set, a member template whose signature names none of the
// class's parameters, a member of a non-dependent base), arguments that
// are not type-dependent. So the object of `.m` below is not
// type-dependent and `.m<N>()` names its member template ([temp.names]/3).
// The forms: `this->f`, `(*this).f`, `f` (implicit `this`), `C::f`,
// `this->C::f`, `Q::f` for a base `Q`, a static member through `this->`
// and `C::`, explicit template arguments, a default member initializer.
// The call is resolved again at instantiation: an explicit specialization
// of a member for one specialization, declared after the definition, is
// the function called. Clang 22 and GCC 16.1 treat every call through
// `this` as type-dependent, so the text decides and no oracle is
// compared. WP-1609.
//@ expect: run
//@ skip(clang): clang 22 treats a call through `this` of the current instantiation as type-dependent
struct R {
  int v;
  template <int N> int m() { return v + N; }
};
struct Q {
  R q(int) { return R{1}; }
  R q(long) { return R{2}; }
};
template <class T> struct A : Q {
  R ov(int) { return R{10}; }
  R ov(long) { return R{20}; }
  static R so(int) { return R{100}; }
  static R so(long) { return R{200}; }
  template <class U> R ft(U) { return R{1000}; }
  template <class U> R ft(U *) { return R{3000}; }
  R ft(int) { return R{2000}; }
  template <class U> U id(U u) { return u; }
  int dmi = ov(1L).m<0>();
  int f() {
    static_assert(sizeof(id('c')) == 1);
    return this->ov(1).m<0>() + (*this).ov(1L).m<0>() + ov(1).m<0>() +
           A::ov(1).m<0>() + this->A::ov(1L).m<0>() + Q::q(1).m<0>() +
           this->Q::q(1L).m<0>() + A::so(1).m<0>() + this->so(1L).m<0>() +
           this->ft<int>(1).m<0>() + ft(1).m<0>() + ft((char *)0).m<0>() +
           (*this).template ft<long>(1).m<0>() + dmi;
  }
};
template <> R A<char>::ov(int) { return R{40}; }
template <> template <> R A<char>::ft<char>(char *) { return R{5000}; }
int main() {
  int base = 10 + 20 + 10 + 10 + 20 + 1 + 2 + 100 + 200 + 1000 + 2000 + 1000 + 20;
  if (A<int>().f() != base + 3000)
    return 1;
  if (A<char>().f() != base + 30 * 3 + 5000)
    return 2;
  return 0;
}
