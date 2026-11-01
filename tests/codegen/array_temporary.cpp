// [expr.type.conv]/2, [dcl.init.list]/3.10, [class.temporary]/4,6:
// array prvalues from a braced list. `A{...}` for an array type `A` is an
// aggregate-initialized temporary; a braced list bound to a reference to
// array materializes one. An array of a class with a non-trivial
// destructor is bound to it (clang `MaybeBindToTemporary` searches
// through the array type) and destroyed element-wise at the end of the
// full-expression, or with the reference that extends it; the element
// destructor is odr-used, so its implicit `noexcept` makes the destroy
// loop `call ... nounwind`.
struct T { T(int); ~T(); int v; };
struct D { int x; };
using A = D*[1];
using TA = T[2];
using U = int[];
using M = T[2][2];
int use(const T*);
int f(const int (&a)[3]);
int ft(const T (&a)[2]);
D d;
D* a1() { return A{&d}[0]; }
void a2() { (void)TA{5, 6}; }
int a3() { return U{5, 6, 7}[2]; }
int a4() { auto&& r = TA{1, 2}; return r[0].v + use(TA{3, 4}); }
int a5() { return M{{1, 2}, {3, 4}}[1][0].v; }
int a6(bool b) { return b ? TA{1, 2}[0].v : use(TA{3, 4}); }
int a7() { int (&&r)[2] = {1, 2}; return r[1]; }
int a8() { return f({1, 2, 3}); }
int a9() { return ft({1, 2}); }
int a10() { const T (&r)[2] = {3, 4}; return r[1].v; }
int a11() { T x(1); return ft({x.v, 2}) + x.v; }
