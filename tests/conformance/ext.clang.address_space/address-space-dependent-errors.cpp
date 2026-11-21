//@ expect: compile-fail
//@ strict-diags
//@ diag: 36:43 error "address space is negative"
//@ diag: 34:30 note "expanded from macro 'AS'"
//@ diag: 37:1 note "in instantiation of template class 'A<-1>' requested here"
//@ diag: 38:49 error "multiple address spaces specified for type"
//@ diag: 34:30 note "expanded from macro 'AS'"
//@ diag: 39:1 note "in instantiation of template class 'B<2>' requested here"
//@ diag: 38:49 warning "multiple identical address spaces specified for type"
//@ diag: 34:30 note "expanded from macro 'AS'"
//@ diag: 40:1 note "in instantiation of template class 'B<1>' requested here"
//@ diag: 43:39 error "field may not be qualified with an address space"
//@ diag: 44:39 error "automatic variable qualified with an address space"
//@ diag: 45:15 note "in instantiation of function template specialization 'e<1>' requested here"
//@ diag: 49:15 error "explicit instantiation of 'f' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 48:23 note "candidate template ignored: substitution failure [with N = 1]: parameter may not be qualified with an address space"
//@ diag: 50:45 error "'address_space' attribute requires an integer constant"
//@ diag: 34:30 note "expanded from macro 'AS'"
//@ diag: 52:1 note "in instantiation of template class 'G<V>' requested here"
//@ diag: 60:3 error "no matching function for call to 'p'"
//@ diag: 57:23 note "candidate template ignored: could not match 'int const __attribute__((address_space(N)))' against 'int'"
//@ diag: 61:3 error "no matching function for call to 'p'"
//@ diag: 57:23 note "candidate template ignored: could not match 'int const __attribute__((address_space(N)))' against '__global int'"
//@ diag: 62:3 error "no matching function for call to 'q'"
//@ diag: 58:23 note "candidate template ignored: couldn't infer template argument 'N'"
// The checks clang makes on a value-dependent address_space(N) (clang
// DependentAddressSpaceType, WP-1811): at instantiation the space must be
// a non-negative integer constant, a second space is an error and the same
// one again a warning, and the parameters, variables and fields that may
// not be in an address space are rejected (a field already at the
// template's definition, a parameter as a substitution failure, an
// automatic variable at instantiation; a static local is fine). Deducing
// N needs an argument in a target address space.
#define AS(n) __attribute__((address_space(n)))

template <int N> struct A { using t = int AS(N); };
A<-1>::t *a1;
template <int N> struct B { using t = AS(1) int AS(N); };
B<2>::t *b2;
B<1>::t *b1;
template <int N> struct C { using t = void AS(N) (); };
C<1>::t *c1;
template <int N> struct D { int AS(N) x; };
template <int N> void e() { int AS(N) x = 0; (void)x; }
template void e<1>();
template <int N> void e2() { static int AS(N) x = 0; (void)x; }
template void e2<1>();
template <int N> void f(int AS(N) p) {}
template void f<1>(int);
template <class T> struct G { using t = int AS(T::v); };
struct V { static constexpr double v = 1.5; };
G<V>::t *g1;
template <int N> struct H { using t = int AS(N) &; };
extern int AS(2) g2;
H<2>::t h2 = g2;

template <int N> void p(const int AS(N) *) {}
template <int N> void q(int AS(N + 1) *) {}
void use(int *x, __attribute__((opencl_global)) int *y, int AS(2) *z) {
  p(x);
  p(y);
  q(z);
}
