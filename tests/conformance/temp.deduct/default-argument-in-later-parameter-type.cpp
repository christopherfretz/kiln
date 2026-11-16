// [temp.deduct.general]/5: a deduced non-type argument is converted to its
// parameter's type once the defaulted parameters it uses are substituted;
// a failure there is a deduction failure. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++20
//@ diag: 11:9 error "no matching function for call to 'f'"
//@ diag: 10:51 note "candidate template ignored: substitution failure [with T = void]: type 'void' cannot be used prior to '::' because it has no members"
template <int> struct K {};
template <class T = void, typename T::type M> int f(K<M>);
int a = f(K<42>{});
struct S { using type = int; };
template <class T = S, typename T::type M> int g(K<M>) { return 0; }
int b = g(K<42>{});
// SFINAE: the substitution failure removes the candidate.
template <class T, typename T::type M> int h(K<M>) { return 1; }
template <class T> int h(...) { return 2; }
int c = h<void>(K<1>{});
