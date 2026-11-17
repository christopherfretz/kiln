// [temp.deduct.call]/4.3 and /5: deduction is done argument by argument,
// so the bases of an argument's class are matched against P on their own,
// not from what another (P, A) pair already deduced. `S<I, I, I>` against
// a class with bases `S<0, 0, 0>` and `S<1, 1, 1>` has two possible deduced
// A and fails, even when the other argument deduced I = 0 (in either
// order, and through a pointer). With I given explicitly, P is
// `S<0, 0, 0>` and the call is valid. Found running GCC's C++ testsuite;
// clang 19 and 22 match the bases from the earlier deductions and accept:
// the text and GCC 16 decide (clang does reject the last case, two
// matching bases with no other argument).
//@ xfail(clang): clang 19 and 22 match the bases from the deductions of earlier arguments
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:12 error "no matching function for call to 'f'"
//@ diag: 23:46 note "candidate template ignored: failed template argument deduction"
//@ diag: 38:3 error "no viable constructor or deduction guide for deduction of template arguments of 'W'"
//@ diag: 37:27 note "candidate template ignored: could not match 'W<T>' against 'D2'"
//@ diag: 37:27 note "implicit deduction guide declared as 'template <class T> W(W<T>) -> W<T>'"
//@ diag: 37:31 note "candidate template ignored: failed template argument deduction"
//@ diag: 37:31 note "implicit deduction guide declared as 'template <class T> W(const B<T> &) -> W<T>'"
template <int I, int J, int K> struct S {};
template <int I, int J> struct S<I, J, 2> : S<I, I, I>, S<J, J, J> {};
template <int I, int J, int K> constexpr int f(S<I, J, K>, S<I, I, I>) { return 1; }
template <int I, int J, int K> constexpr int g(S<I, I, I>*, S<I, J, K>) { return 1; }
constexpr int g(...) { return 2; }

S<0, 0, 0> s0;
S<0, 1, 2> s2;
static_assert(f<0>(s0, s2) == 1);
static_assert(g<0>(&s2, s0) == 1);
static_assert(g(&s2, s0) == 2);
void t() { f(s0, s2); }

// Class template argument deduction through two matching bases fails too.
template <class T> struct B {};
struct D2 : B<int>, B<char> {};
template <class T> struct W { W(const B<T>&); };
W w(D2{});
