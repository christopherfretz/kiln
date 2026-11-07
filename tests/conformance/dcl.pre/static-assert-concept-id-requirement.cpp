// [dcl.pre]/12, [temp.names]/8: a failed requirement prints a concept-id
// as its qualifier as written, the concept's name and its arguments,
// canonical (a local alias names its target), as clang does; kiln printed
// `...` for it (stdexec's `!__merror<_Completions>`, whose build-failure
// test looks for the error type's name in the output; WP-789).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:17 error "static assertion failed due to requirement '!small<char> || ns::same<char, long>'"
//@ diag: 27:15 note "in instantiation of function template specialization 'f<char>' requested here"
//@ diag: 25:17 error "static assertion failed due to requirement '!merror<err<why>>': e"
//@ diag: 25:17 error "static assertion failed due to requirement '!merror<err<int>>': e"
//@ diag: 28:15 note "in instantiation of function template specialization 'f<int>' requested here"
namespace ns {
template <class T, class U> concept same = __is_same(T, U);
}
template <class T> concept small = sizeof(T) < 4;
template <class... W> struct err {};
struct why;
template <class T> concept merror = __is_same(T, err<why>) || __is_same(T, err<int>);
template <class T> struct pick { using type = err<why>; };
template <> struct pick<int> { using type = err<int>; };
template <class C> void f() {
  static_assert(!small<C> || ns::same<C, long>);
  using completions = typename pick<C>::type;
  static_assert(!merror<completions>, "e");
}
template void f<char>();
template void f<int>();

