//@ expect: ast
//@ oracle: clang
// [over.match.call]/2 (spec/sema-overload.md §2.3): calls of overloaded
// member functions through `.`, `->` and an implicit `this->`, with the
// implicit object argument deciding between const and non-const members.
struct S {
  int f(int);
  double f(double);
  int g() const;
  int g();
  static int h(int);
  int h(double);
  int use() const { return g() + h(1); }
  int use2() { return g() + S::f(2) + h(3); }
};
int t(S s, const S& cs, S* p) {
  return s.f(1) + p->f(2.0) + cs.g() + s.g() + s.h(1) + (s.f)(3);
}
