// Shapes whose x86-64 SysV lowering depends on the AVX ABI level (clang
// `X86AVXABILevel`; spec/landed/abi/WP-1418.md), shared by the
// ir-avx-abi-x86_64-*.cpp tests: each compiles this file under one set of
// target flags and must equal clang's IR.
typedef float f4 __attribute__((vector_size(16)));
typedef float f8 __attribute__((vector_size(32)));
typedef double d4 __attribute__((vector_size(32)));
typedef long long l4 __attribute__((vector_size(32)));
typedef float f16 __attribute__((vector_size(64)));
typedef double d8 __attribute__((vector_size(64)));
typedef __int128 q2 __attribute__((vector_size(32)));
typedef double e3 __attribute__((ext_vector_type(3)));

// One vector, in each wrapper: in registers up to the native size.
struct W { f8 v; };
struct W16 { f16 v; };
union U { d4 v; };
union UMix { f8 v; d4 w; float a[8]; };
union UHalf { f8 v; f4 h; };
struct A1 { l4 v[1]; };
struct Nest { struct W in; };
struct Empty {};
struct Derived : Empty { d4 v; };
struct FromW : W {};
// Mixed aggregates that stay in memory at every level.
struct Mix { f8 v; int i; };
struct Pair { f4 a, b; };
struct Floats { float a[8]; };
struct Two { f8 a, b; };
// Unaffected by AVX.
struct Small { f4 v; };
struct Q { q2 v; };
// A class that is not trivially copyable is passed by address.
struct NonTrivial { f8 v; NonTrivial(const NonTrivial &); ~NonTrivial(); };

f8 ext_f8(f8, int, f8);
struct W ext_w(struct W, struct W16);
f16 ext_f16(f16);
void ext_va(int, ...);

f8 bare(f8 a) { return ext_f8(a, 1, a); }
d4 bare_d(d4 a, e3 b) { return a + (d4){b.x, b.y, b.z, 0}; }
f16 bare16(f16 a) { return ext_f16(a); }
q2 int128_vec(q2 a) { return a; }
struct W wrap(struct W a, struct W16 b) { return ext_w(a, b); }
struct W16 wrap16(struct W16 a) { return a; }
union U un(union U a) { return a; }
union UMix un_mix(union UMix a) { return a; }
union UHalf un_half(union UHalf a) { return a; }
struct A1 arr1(struct A1 a) { return a; }
struct Nest nest(struct Nest a) { return a; }
struct Derived derived(struct Derived a) { return a; }
struct FromW from_w(struct FromW a) { return a; }
struct Mix mix(struct Mix a) { return a; }
struct Pair pair(struct Pair a) { return a; }
struct Floats floats(struct Floats a) { return a; }
struct Two two(struct Two a) { return a; }
struct Small small(struct Small a) { return a; }
struct Q q(struct Q a) { return a; }
double _Complex cplx(double _Complex a) { return a; }
__int128 i128(__int128 a) { return a; }
NonTrivial non_trivial(NonTrivial a) { return a; }

// Eight `double`s take the SSE registers, so the vectors go on the stack.
void many(double, double, double, double, double, double, double, f8, struct W);
void call_many(f8 a, struct W w) { many(0, 0, 0, 0, 0, 0, 0, a, w); }

// Through `...` a wide vector is unnamed: never in a register.
void pass_va(f8 a, struct W w) { ext_va(2, a, w); }
f8 read_va(int n, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, n);
  f8 v = __builtin_va_arg(ap, f8);
  struct W w = __builtin_va_arg(ap, struct W);
  __builtin_va_end(ap);
  return v + w.v;
}

// Indirect calls, members, lambdas and thunks use the same lowering.
f8 through_ptr(f8 (*p)(f8, int, f8), f8 a) { return p(a, 2, a); }
struct M {
  d4 m(d4 a, union U b) { return a + b.v; }
  static struct W s(struct W a) { return a; }
};
d4 call_m(M &m, d4 a, union U b) { return m.m(a, b); }
struct W call_s(struct W a) { return M::s(a); }
d4 call_mp(M &m, d4 (M::*p)(d4, union U), d4 a, union U b) { return (m.*p)(a, b); }
f8 lambda(f8 a) {
  auto l = [](f8 x, struct W w) { return x + w.v; };
  return l(a, W{a});
}
struct B1 { virtual f8 g(f8 a) { return a; } };
struct B2 { virtual f8 h(f8 a) { return a; } };
struct D : B1, B2 { f8 h(f8 a) override { return a * 2; } };
D *make_d() { return new D; }
