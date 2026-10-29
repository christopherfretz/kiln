// [temp.spec.partial.general]/9.2: a partial specialization must be more
// specialized than the primary template.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:36 error "class template partial specialization does not specialize any template argument; to define the primary template, remove the template argument list"
//@ diag: 11:36 error "redefinition of 'A'"
//@ diag: 9:42 note "previous definition is here"
//@ diag: 13:41 error "variable template partial specialization does not specialize any template argument; to define the primary template, remove the template argument list"
template <class T, class U = int> struct A {};
template <class T> struct A<T, int> {};
template <class T, class U> struct A<T, U> {};
template <class T, int N> constexpr int v = 0;
template <class T, int N> constexpr int v<T, N> = 1;
