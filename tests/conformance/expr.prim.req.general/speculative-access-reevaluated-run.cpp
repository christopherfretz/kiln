// [expr.prim.req.general]/5, [temp.res.general]/6, [class.access]/2: in a
// class template's pattern a requirement's access may be granted only
// because the class may instantiate to a befriended specialization
// (`friend struct A<int>;`) or derive from the naming class through a
// dependent base. Such a requires-expression decides nothing in the
// pattern: each specialization evaluates it, and a check that fails there
// makes it false, never a hard error (clang `TransformRequiresExpr`
// replays the body's dependent access checks under a SFINAE trap).
//@ expect: run
//@ stdout: 1 0 1 0 1 0 1 0 1 0 1 0
extern "C" int printf(const char *, ...);
template <class T> struct A;
class B {
  friend struct A<int>;
  int n = 5;
  static constexpr int sn = 5;
public:
  B() {}
};
template <class T> struct A {
  // Non-dependent requirements, evaluated where the requires-expression
  // is formed outside a template.
  static constexpr bool v = requires { B::sn; };
  static int g() {
    if constexpr (requires { B::sn; }) return 1;
    else return 0;
  }
  static int h() { return requires { B().n; } ? 1 : 0; }
  // A local parameter, and a function parameter.
  static constexpr bool w = requires(const B &x) { x.n; };
  static int k(const B &x) { return requires { x.n; } ? 1 : 0; }
  // Nested in another requires-expression.
  static constexpr bool u = requires { requires requires { B::sn; }; };
};
struct P {
protected:
  static constexpr int pn = 1;
};
struct Q {};
template <class T> struct D : T {
  static constexpr bool v = requires { P::pn; };
};
static_assert(A<int>::v && !A<long>::v);
static_assert(A<int>::w && !A<long>::w);
static_assert(A<int>::u && !A<long>::u);
static_assert(D<P>::v && !D<Q>::v);
int main() {
  B b;
  printf("%d %d %d %d %d %d %d %d %d %d %d %d\n", A<int>::g(), A<long>::g(),
         A<int>::h(), A<long>::h(), A<int>::k(b), A<long>::k(b), A<int>::v,
         A<long>::v, A<int>::w, A<long>::w, D<P>::v, D<Q>::v);
}
