// [class.conv.fct]/1, [dcl.fct]/6 (P0847): a conversion function may have
// an explicit object parameter (it has no non-object parameters). It is
// still a conversion function: user-defined conversion sequences, value-
// category and cv overloads on the object parameter ([over.match.funcs]/4),
// a deduced `this auto&&` / `this Self&&` object, a conversion template,
// `explicit`, `operator auto`, a derived-class object and a by-value
// object, all evaluated in a constant expression. Matches clang 19 and 22.
//@ expect: compile-ok
//@ strict-diags
struct S { int v = 3; constexpr operator int(this const S& self) { return self.v; } };
static_assert(int(S{}) == 3);
constexpr S s{7};
constexpr int i = s;
static_assert(i == 7);
struct R {
  constexpr operator int(this R& self) { return 1; }
  constexpr operator int(this R&& self) { return 2; }
};
constexpr int lr() { R r; int x = r; return x; }
constexpr int rr() { return R{}; }
static_assert(lr() == 1 && rr() == 2);
struct G {
  int v;
  template <class Self> constexpr operator int(this Self&& self) {
    return __is_lvalue_reference(Self) ? self.v : -self.v;
  }
};
constexpr int g1() { G g{4}; int x = g; return x; }
constexpr int g2() { return G{4}; }
static_assert(g1() == 4 && g2() == -4);
struct T {
  template <class U> constexpr operator U(this const T&) { return U(5); }
};
constexpr long tl = T{};
constexpr double td = T{};
static_assert(tl == 5 && td == 5.0);
struct E { explicit constexpr operator bool(this E) { return true; } };
static_assert(static_cast<bool>(E{}));
constexpr bool eb() { if (E{}) return true; return false; }
static_assert(eb());
struct A { constexpr operator auto(this const A&) { return 9; } };
constexpr int ai = A{};
static_assert(ai == 9);
struct B { int v; constexpr operator int(this auto&& self) { return self.v; } };
constexpr int bi = B{8};
static_assert(bi == 8);
struct D : B {};
constexpr int di = D{{6}};
static_assert(di == 6);
struct H { constexpr operator int(this H self) { return 11; } };
constexpr int hi = H{};
static_assert(hi == 11);
struct CQ { constexpr operator int(this const CQ&) { return 1; } constexpr operator int(this CQ&) { return 2; } };
constexpr int cq() { CQ a; const CQ b; int x = a; int y = b; return x * 10 + y; }
static_assert(cq() == 21);
