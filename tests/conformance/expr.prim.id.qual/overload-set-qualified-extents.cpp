// [expr.prim.id.qual]/1, [over.match.call]/2: a qualified name that
// names an overload set of static and non-static member functions,
// resolved by the call, begins at its qualifier (spec/ast.md §21.2):
// the static winner is a qualified reference (`Q::f(*this, 1)`), the
// non-static one an implicit `this->Q::f(2)`; also a template-id
// (`Q::t<int>`) and a qualifier naming a base (`Q::` in `D`).
//@ expect: compile-ok
struct Q {
  static int f(const Q &, int);
  int f(int) const;
  int h(int) const;
  static int h(const Q &, int, int);
  template <class T> static int t(const Q &, T);
  template <class T> int t(T) const;
  int g() const {
    return Q::f(*this, 1) + Q::f(2) + Q::h(3) + Q::h(*this, 4, 5) + Q::t<int>(*this, 6) +
           Q::t<int>(7) + Q::t(8);
  }
};
struct D : Q {
  int k() const { return Q::f(*this, 1) + D::f(2) + Q::template t<long>(9); }
};
template <class T> struct TQ {
  static int f(const TQ &, int);
  int f(int) const;
  int g() const { return TQ::f(*this, 1) + TQ<T>::f(2); }
};
int use(const D &d, const TQ<int> &q) { return d.g() + d.k() + q.g(); }
