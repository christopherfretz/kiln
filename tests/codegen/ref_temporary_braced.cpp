// Lifetime-extended temporaries initialized by a braced list, and ones
// reached only through a constant pointer (WP-224): `const int& a = {}`
// materializes a `const int` directly from the list, so its `_ZGR` is
// `constant`, while `{7}` materializes the converted `int` operand of a
// no-op cast and stays writable. A temporary extended by a reference
// member of a constexpr aggregate, or named by a constexpr pointer, is
// emitted from the frozen value (codegen `ptr_base`, `PtrBase::Temporary`)
// with the same constness rule.
struct P { int x, y; };
struct M { int x; mutable int y; };
struct R { const int& r; };
struct RP { const P& r; };
struct RA { const int (&r)[2]; };
const int& a = {};
const int& b = 5;
const int& c = {7};
const P& d = {1, 2};
const P& e = P{3, 4};
constexpr const int& f = {};
const M& g = {1, 2};
const int (&h)[2] = {1, 2};
constexpr R x{{}};
constexpr R y{5};
constexpr RP z{{1, 2}};
constexpr RP w{P{1, 2}};
constexpr RA q{{3, 4}};
constexpr const int& k = {};
constexpr const int* pk = &k;
constexpr const double& dd = 1;
constexpr const double* pd = &dd;
const int* const* ptrs() {
  static const int* v[] = {&a, &b, &c, &d.x, &e.x, &f, &g.x, &h[0],
                           &x.r, &y.r, &z.r.x, &w.r.x, &q.r[1], pk};
  return v;
}
const double* dptr() { return pd; }
