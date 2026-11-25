// [temp.spec.partial.match]/1, [temp.deduct.general]/8: a partial-
// specialization ambiguity first met while substituting into a candidate's
// signature is a substitution failure (the candidate is removed silently),
// and the specialization is remembered as failed; the later use of the
// same specialization outside any substitution is still ill-formed and
// is diagnosed there, as it would have been had it come first (WP-3283;
// spec/sema-templates.md §5.7.3 "Hard use of a silent failure"). clang 19
// diagnoses neither use, clang 22 not the variable's.
//@ expect: compile-fail
//@ strict-diags
//@ oracle: gcc
//@ xfail(clang): clang 19 reports neither ambiguity after the SFINAE failure, clang 22 not the variable template's
//@ diag: 32:13 error "ambiguous partial specializations of 'X<int, int>'"
//@ diag: 20:27 note "partial specialization matches [with T = int]"
//@ diag: 21:27 note "partial specialization matches [with U = int]"
//@ diag: 33:9 error "ambiguous partial specializations of 'v<int, int>'"
//@ diag: 23:34 note "partial specialization matches [with T = int]"
//@ diag: 24:34 note "partial specialization matches [with U = int]"
template <class T, class U> struct X;
template <class T> struct X<T, int> {};
template <class U> struct X<int, U> {};
template <class T, class U> constexpr int v = 0;
template <class T> constexpr int v<T, int> = 1;
template <class U> constexpr int v<int, U> = 2;

template <class T> auto f(T*) -> decltype(sizeof(X<T, int>)) { return 0; }
template <class T> char f(...) { return 0; }
template <class T> auto g(T*) -> decltype(v<T, int>) { return 0; }
template <class T> char g(...) { return 0; }
static_assert(sizeof(f<int>(nullptr)) == 1);
static_assert(sizeof(g<int>(nullptr)) == 1);
X<int, int> x;
int y = v<int, int>;
