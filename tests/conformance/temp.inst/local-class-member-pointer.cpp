// [expr.unary.op]/3, [temp.inst]: `&L::m` naming a member of a local
// class of a function template is a pointer to member in every
// instantiation (the qualifier as written survives substitution).
//@ expect: run
//@ strict-diags
template <class T> T g(T t) {
  struct L {
    T v;
    T get() const noexcept { return v; }
    static T s() { return 1; }
  };
  T L::*pm = &L::v;
  auto pf = &L::get;
  T (*ps)() = &L::s;
  static_assert(noexcept((L{}.*pf)()));
  L l{t};
  return (l.*pf)() + l.*pm + ps();
}
int main() { return g(4) == 9 ? 0 : 1; }
