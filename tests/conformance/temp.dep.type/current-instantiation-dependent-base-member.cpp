// [temp.dep.type]/6, [temp.dep.expr]/3: a qualified name `D::x` whose
// nested-name-specifier is the current instantiation, when lookup finds
// nothing and the class has a dependent base, names a member of an unknown
// specialization: it is looked up again at instantiation (and is an
// implicit member access there). libc++'s test support does
// `return (UnwrapAdaptor::c);` for a protected base member (WP-309).
//@ expect: run
//@ strict-diags
template <class A>
struct U : A {
  decltype(auto) get() { return (U::c); } // `int &`
  decltype(auto) get2()
    requires requires { U::c; }
  {
    return (U::comp);
  }
  int get3() { return U<A>::c; }
  static int get4() { return U::s; }
};
struct Q {
  static constexpr int s = 5;

protected:
  int c = 3;
  int comp = 4;
};
int main() {
  U<Q> u;
  u.get() = 10;
  return u.get() + u.get2() + u.get3() + U<Q>::get4() == 29 ? 0 : 1;
}
