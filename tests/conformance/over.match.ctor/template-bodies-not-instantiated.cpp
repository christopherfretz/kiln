// [temp.inst]/11, [over.match.funcs]/8: overload resolution instantiates
// only the declarations of function template specializations (their
// signatures; a noexcept-specifier only when needed, [temp.inst]/14), never
// their definitions: a conversion function template, a constructor
// template and a function template that each lose to a non-template
// keep their ill-formed bodies (and exception specifications)
// uninstantiated. Includes the CWG2327 candidates of an unrelated class
// (`E e(d)`: `D::operator E()` competes with `E(const D&)` and loses to
// the constructor). Matches clang 19.
//@ expect: compile-ok
//@ strict-diags
template <class T> constexpr bool never = false;
struct D {
  D() = default;
  D(const D&) = default;
  template <class T> operator T() const noexcept(T::nope) {
    static_assert(never<T>, "conversion body");
    return T::nope2;
  }
};
struct E {
  E(const D&) {}
  template <class T> E(T, int) { static_assert(never<T>, "constructor template body"); }
};
struct F {
  F(int) {}
  template <class T> F(T*) { static_assert(never<T>, "constructor template body"); }
};
struct G { G(const D&) {} };
void g(int, int) {}
template <class T> void g(T, long) { static_assert(never<T>, "function template body"); }
int h(int) { return 1; }
template <class T> auto h(T) noexcept(never<T>) -> int {
  static_assert(never<T>, "function template body");
  return 0;
}
struct DD : D {};
int main() {
  D d;
  E e(d);
  E e2 = d;
  E e3{d};
  G gg{d};
  F f(0);
  g(1, 2);
  DD dd;
  D x = dd;
  D y(dd);
  D z{dd};
  (void)e; (void)e2; (void)e3; (void)gg; (void)f; (void)x; (void)y; (void)z;
  return h(1) - 1;
}
