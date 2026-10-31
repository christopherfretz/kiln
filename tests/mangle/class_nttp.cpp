// Class-type and placeholder-type non-type template arguments (WP-117):
// `Xtl…E` values, `so` subobject pointers, `di` union members, `_ZTA`
// template parameter objects and the `Tn`/`Tp` template-param-decl prefix.
namespace a {
struct S { int a; int b; };
struct E {};
struct Z { int a; int b; int c; };
struct N { S s; int k; };
struct B : S { int c; };
struct Arr { int v[3]; };
struct Ch { char c; bool f; unsigned u; long l; };
struct F { double d; float f; };
union U { int i; float f; };
struct P { const int *p; };
struct R { int &r; };
struct Bits { int x : 3; int : 0; int y : 4; };
enum class Col { red, green = 5 };
struct En { Col c; };
struct MP { int S::*m; };
struct Neg { int a; long b; };
int gx = 0;
int garr[4] = {};
template <auto V> void f() {}
template <S V> void g() {}
template <S V> struct T { static void h() {} };
template <S V> int vt = V.a;
void use() {
  f<S{1, 2}>();
  f<S{1, 0}>();
  f<S{0, 0}>();
  f<S{0, 3}>();
  f<E{}>();
  f<Z{1, 0, 0}>();
  f<N{{1, 2}, 3}>();
  f<N{{0, 0}, 3}>();
  f<N{{1, 0}, 0}>();
  f<B{{1, 2}, 3}>();
  f<B{{0, 0}, 0}>();
  f<Arr{{1, 2, 3}}>();
  f<Arr{{1, 0, 0}}>();
  f<Arr{{0, 0, 0}}>();
  f<Ch{'a', true, 7u, -3l}>();
  f<F{1.5, 2.0f}>();
  f<F{0.0, 0.0f}>();
  f<U{3}>();
  f<U{.f = 1.0f}>();
  f<U{}>();
  f<P{&gx}>();
  f<P{nullptr}>();
  f<P{&garr[2]}>();
  f<R{gx}>();
  f<Bits{1, 2}>();
  f<En{Col::green}>();
  f<En{Col::red}>();
  f<MP{&S::b}>();
  f<MP{nullptr}>();
  f<Neg{-1, -2}>();
  g<S{4, 5}>();
  T<S{6, 7}>::h();
  (void)vt<S{8, 9}>;
}
template <S V> const S &addr() { return V; }
void use2() { addr<S{2, 3}>(); addr<S{0, 0}>(); }
} // namespace a
namespace b {
struct S { int a; int b; };
int gx;
template <auto V> void f() {}
template <auto &V> void fr() {}
template <auto *V> void fp() {}
template <decltype(auto) V> void fd() {}
template <class T, T V> void ft() {}
void use() { f<5>(); f<'c'>(); f<&gx>(); f<nullptr>(); f<true>(); fr<gx>(); fp<&gx>(); fd<5>(); fd<(gx)>(); ft<int, 3>(); ft<S, S{1,2}>(); f<1.5>(); }
} // namespace b
namespace c {
struct S { int a; int b; };
template <class T, T V> void ft(T) {}
template <class U, S V> void h() {}
template <class T, T V> struct C { static void m() {} };
template <auto V> struct D { static void m() {} };
template <class T, T... V> void pk() {}
template <auto... V> void pa() {}
void use() { ft<S, S{1,2}>(S{}); h<S, S{1,2}>(); h<int, S{1,2}>(); C<S, S{3,4}>::m(); D<S{5,6}>::m(); D<5>::m(); pk<int, 1, 2>(); pa<1, 'a'>(); pa<>(); pk<int>(); }
} // namespace c
namespace d {
template <int N> int vi = N;
int g = 1;
void u() { (void)vi<3>; (void)g; }
int w() { return vi<4>; }
} // namespace d
namespace e {
struct S { int a; int b; };
struct Q { S s[2]; int t; };
union V { int i; S s; };
S gs = {};
Q gq = {};
V gv = {};
int garr[4] = {};
struct P { const int *p; };
struct R { const int &r; };
struct PS { const S *p; };
template <auto V> void f() {}
template <auto &V> void fr() {}
template <const int *V> void fp() {}
template <const int &V> void fq() {}
void use() {
  f<&gs.b>();
  f<&gs.a>();
  f<garr + 4>();
  f<garr + 1>();
  f<&garr>();
  f<&gq.s[1].b>();
  f<&gq.s[1]>();
  f<&gq.t>();
  f<&gv.i>();
  fr<gs.b>();
  fr<gq.s[0]>();
  fr<garr[3]>();
  fp<&gs.b>();
  fp<garr + 2>();
  fq<gs.b>();
  fq<garr[1]>();
  f<P{&gs.b}>();
  f<P{garr + 4}>();
  f<R{gs.b}>();
  f<R{garr[3]}>();
  f<PS{&gq.s[1]}>();
}
} // namespace e
