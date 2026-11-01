// [expr.sub], [expr.const]: a multidimensional class-typed member array read through a
// const access path (const reference, pointer to const, const member function) can be
// subscripted at any index in constant evaluation (cv-qualified element types designate the
// same array)
//@ expect: compile-ok
//@ strict-diags
struct P { int x = 0; };
struct S {
  P p[2][3];
  constexpr int rd(const S& o) const { return o.p[1][2].x; }
  constexpr int rd2() const { return p[1][2].x; }
};
constexpr int f() { S b{}; b.p[1][2].x = 7; return b.rd(b); }
static_assert(f() == 7);
constexpr int g() { S b{}; b.p[1][2].x = 7; return b.rd2(); }
static_assert(g() == 7);
constexpr int k() { S b{}; const S& r = b; b.p[1][2].x = 7; return r.p[1][2].x; }
static_assert(k() == 7);
constexpr int m() { S b{}; const S* r = &b; b.p[1][1].x = 7; const P* q = &r->p[1][0]; return (q + 1)->x + (q + 2 - q); }
static_assert(m() == 9);
