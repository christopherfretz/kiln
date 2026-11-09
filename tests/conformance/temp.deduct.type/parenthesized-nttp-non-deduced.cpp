// [temp.deduct.type]/5.3, CWG526: a non-type template argument that is not
// just the parameter name, such as '(N)', is a non-deduced context, so a call
// cannot deduce it and a partial specialization using it is unusable. A bare
// 'N' still deduces, and explicit arguments still work. Found by the clang
// negative-test run (WP-912); fixed by WP-1017.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:18 error "no matching function for call to 'f'"
//@ diag: 14:23 note "candidate template ignored: couldn't infer template argument 'N'"
//@ diag: 19:25 error "class template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 19:15 note "non-deducible template parameter 'N'"
template <int> struct S {};
template <int N> void f(S<(N)>);
void g(S<3> s) { f(s); }
template <int N> void h(S<N>);
void k(S<3> s) { h(s); f<3>(s); }
template <int A, int B> struct P {};
template <int N> struct P<(N), 0> {};
template <int N> struct P<N, 1> {};
P<2, 1> p1;
template <int N> constexpr int v(S<((N))> *) { return N; }
static_assert(v<4>(nullptr) == 4);
