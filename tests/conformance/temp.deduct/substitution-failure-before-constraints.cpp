// [temp.deduct.general]/5, [temp.deduct.call]/10 (CWG1391), clang 19's
// order: a non-dependent argument conversion is checked first, then the
// deduced arguments are substituted into the function type (a failure is
// a deduction failure), and only then are the associated constraints
// checked. A candidate whose substitution fails and whose constraints are
// unsatisfied reports the substitution failure; one whose non-dependent
// conversion fails reports that. A viable overload is still chosen.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:10 error "no matching function for call to 'f'"
//@ diag: 24:18 note "candidate template ignored: substitution failure [with T = int]: type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 29:10 error "no matching function for call to 'h'"
//@ diag: 28:6 note "candidate function template not viable: no known conversion from 'std::nullptr_t' to 'int' for 2nd argument"
//@ requires: clang<22
// clang 19 spelling; substitution-failure-before-constraints-clang22.cpp has clang 22's (WP-394).
template <class T> constexpr bool small = sizeof(T) < 2;
// `p(1)`: the template fails substitution, the non-template is chosen.

template <class T> requires small<T> typename T::type p(T);
int p(long);
static_assert(__is_same(decltype(p(1)), int));

template <class T> requires small<T>
typename T::type f(T);
int a = (f(1), 0);

template <class T> requires small<T>
void h(T, int);
int c = (h(1, nullptr), 0);
