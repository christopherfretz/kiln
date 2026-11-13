// [expr.prim.lambda.closure]/3 (P0170R1), [basic.types.general]/10,
// [dcl.constexpr]/3: from C++17 a closure type is a literal type when its
// members are, with no constructor needed (clang `CXXRecordDecl::isLiteral`),
// so a constexpr function may return or take one. A reference capture is a
// reference or no member at all, literal whatever it refers to; a copy, an
// init-capture, `this` and `*this` of a literal type are literal; a
// captureless and a generic lambda too. The return type is checked at the
// definition (WP-1123); WP-1225 fixed captured closures rejected there.
//@ expect: run
//@ std: c++17
//@ strict-diags
//@ stdout: 1|2|3|4|5|6|7|8|9|10|11|12|13
extern "C" int printf(const char *, ...);

struct NL { NL() : v(1) {} ~NL() {} int v; };

struct S {
  int m;
  constexpr auto by_ref(NL &n) const { return [&n] { return n.v; }; }
  constexpr auto by_copy(int r) const { return [r] { return r; }; }
  constexpr auto init(int r) const { return [y = r + 1] { return y; }; }
  constexpr auto init_ref(NL &n) const { return [&y = n] { return y.v + 3; }; }
  constexpr auto self() const { return [this] { return m; }; }
  constexpr auto self_copy() const { return [*this] { return m + 1; }; }
  constexpr auto none() const { return [] { return 7; }; }
  constexpr auto generic(int r) const { return [r](auto x) { return r + x; }; }
  constexpr auto dflt_copy(int r) const { return [=] { return r; }; }
  constexpr auto dflt_ref(NL &n) const { return [&] { return n.v + 9; }; }
  // The shape found in real code: a static member forwarding a
  // reference to a non-literal type into the closure.
  static constexpr auto mv(NL &&from) { return [&from](int &p) { p = from.v + 10; }; }
};

constexpr int take(decltype(S{}.by_copy(0)) l) { return l(); }

// Constant evaluation of the returned closures.
constexpr S s5{5};
static_assert(S{0}.by_copy(2)() == 2, "");
static_assert(S{0}.init(2)() == 3, "");
static_assert(s5.self()() == 5, "");
static_assert(S{5}.self_copy()() == 6, "");
static_assert(S{0}.none()() == 7, "");
static_assert(S{0}.generic(4)(4) == 8, "");
static_assert(take(S{0}.by_copy(12)) == 12, "");
constexpr auto cv = [x = 13] { return x; };
static_assert(cv() == 13, "");

int main() {
  NL n;
  S s{5};
  int p = 0;
  S::mv(static_cast<NL &&>(n))(p);
  printf("%d|%d|%d|%d|%d|%d|%d|%d|%d|%d|%d|%d|%d\n", s.by_ref(n)(), s.by_copy(2)(),
         s.init(2)(), s.init_ref(n)(), s.self()(), s.self_copy()(), s.none()(),
         s.generic(4)(4), s.dflt_copy(9)(), s.dflt_ref(n)(), p, take(s.by_copy(12)), cv());
}
