// A value-dependent address_space(N) (clang DependentAddressSpaceType,
// WP-1811) in the IR: each instantiation takes its space, and the
// template's signature mangles the space as `U2ASI <expression> E`
// followed by the pointee's own cv-qualifiers (the cv-qualifiers written
// outside the attribute are not part of the mangling), with the
// qualified type and a `T_` inside the expression as substitution
// candidates; a deduced N comes from the argument's space.
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -ftrivial-auto-var-init=uninitialized
#define AS(n) __attribute__((address_space(n)))
struct S {};
typedef const int CI;

template <int N> int read(AS(N) int *p) { return *p; }
int use(AS(3) int *p) { return read(p); }
template <int N> void g(int AS(N) *, int (*)[N]) {}
template void g<2>(int AS(2) *, int (*)[2]);
template <int N> void h(decltype((int AS(N + 1) *)0)) {}
template void h<4>(int AS(5) *);
template <class T, int N> void k(T AS(N) *) {}
template void k<char, 7>(char AS(7) *);
template <int N> void a(const int AS(N) *) {}
template void a<1>(const int AS(1) *);
template <int N> void b(int AS(N) * AS(N + 1) *) {}
template void b<1>(int AS(1) * AS(2) *);
template <int N> void c(volatile int AS(N) const *) {}
template void c<1>(const volatile int AS(1) *);
template <int N, class T> void d(T AS(N) *) {}
template void d<1, const int>(const int AS(1) *);
template <int N> using sp = int AS(N);
template <int N> void e(sp<N> *) {}
template void e<1>(int AS(1) *);
template <int N> void f(int AS(N) (*)[3]) {}
template void f<1>(int AS(1) (*)[3]);
template <int N> void r(int AS(N) &) {}
template void r<1>(int AS(1) &);
template <int N> void t(CI AS(N) *) {}
template void t<1>(const int AS(1) *);
template <int N> void m(int AS(N) *, int AS(N) *, int AS(N) &) {}
template void m<1>(int AS(1) *, int AS(1) *, int AS(1) &);
template <int N> void n(const int AS(N) *, const int AS(N) *, int AS(N) *) {}
template void n<1>(const int AS(1) *, const int AS(1) *, int AS(1) *);
template <int N> void o(S AS(N) *, S *) {}
template void o<1>(S AS(1) *, S *);
template <int N> void p(int *AS(N) *) {}
template void p<1>(int *AS(1) *);
template <int N> void q(int *const AS(N) *) {}
template void q<1>(int *const AS(1) *);
template <int N> void u(int AS(N) * AS(1) *) {}
template void u<2>(int AS(2) * AS(1) *);
template <int N> void v(int AS(1) * AS(N) *) {}
template void v<2>(int AS(1) * AS(2) *);
template <int N> void w(int AS(N) *, char AS(N) *, S *, S *) {}
template void w<1>(int AS(1) *, char AS(1) *, S *, S *);
template <int N> void x(int (*)[N], S *, S *, int AS(N) *, S *) {}
template void x<1>(int (*)[1], S *, S *, int AS(1) *, S *);
template <int N> struct box { static int AS(N) v; };
template <int N> int AS(N) box<N>::v = N;
int load() { return box<4>::v; }
