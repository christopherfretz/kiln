// [temp.arg.nontype]/1, [temp.param]/8 at `-g` (WP-1424; found compiling
// the libc++ tests with -g): a class-type template argument's
// DITemplateValueParameter `value:` is a constant of the class's struct
// type (`%"struct.n::ic" undef` for an empty class). When the only path to
// it is a local variable's `#dbg_declare` record and the class is only
// constant-evaluated, the struct's definition must still be printed, or
// the backend rejects the text ("invalid type for undef constant").
// Covers an empty class, members (int, pointer, reference to an object
// with linkage, nested class, array, bit-fields), a union with an active
// member, an `auto` parameter deduced to a class type, a captureless
// lambda, and arguments of class, function, variable and alias templates.
//@ expect: run
//@ args: -g
//@ stdout: 1 7 2 2 3 4 5 6 9 10
extern "C" int printf(const char *, ...);
namespace n {
template <class T, T v> struct ic {
  constexpr T operator()() const noexcept { return v; }
};
using ft = ic<bool, false>;
}
template <auto F> struct W {
  constexpr bool operator()() const { return !F(); }
};
template <auto F> constexpr W<F> nf() { return {}; }

int gi = 2;
struct In { int i; };
struct P { int a; const int *p; In in; int arr[2]; };
struct R { const int &r; };
struct B { int x : 3; int y : 5; };
union U { int i; float f; };
struct E {};
template <P p> struct XP { int get() const { return p.a + *p.p + p.in.i + p.arr[1]; } };
template <R r> struct XR { int get() const { return r.r; } };
template <B b> struct XB { int get() const { return b.x + b.y; } };
template <U u> struct XU { int get() const { return u.i; } };
template <auto L> struct XL { int get() const { return L(); } };
template <E e> struct XE { int get() const { return 6; } };
template <B b> constexpr int vt = b.x * 3;
template <U u> using AU = XU<u>;
template <P p> int fp() { XP<p> x; return x.get(); }

int main() {
  auto w = nf<n::ft{}>();
  XP<P{1, &gi, {1}, {0, 3}}> xp;
  XR<R{gi}> xr;
  XB<B{1, 1}> xb;
  XU<U{.i = 3}> xu;
  constexpr auto lam = [] { return 4; };
  XL<lam> xl;
  XE<E{}> xe;
  AU<U{.i = 5}> au;
  int v = vt<B{3, 0}>;
  printf("%d %d %d %d %d %d %d %d %d %d\n", w() ? 1 : 0, xp.get(), xr.get(), xb.get(), xu.get(), xl.get(),
         au.get(), xe.get(), v, fp<P{2, &gi, {2}, {0, 4}}>());
}
