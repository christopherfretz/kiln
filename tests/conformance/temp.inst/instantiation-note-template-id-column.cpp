// [temp.point]/4, [temp.inst]/2: a class template specialization named
// by a dependent `X<args>::` component of a nested-name-specifier is
// implicitly instantiated at substitution, and the "in instantiation of
// template class" note points at that component's start: its template
// name, or its `template` keyword when one is written (clang 19
// RequireCompleteDeclContext, the last qualifier name's location), not
// at the declarator or the qualified name after it (WP-307).
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 35:46 error "static assertion failed due to requirement 'sizeof(char) == 0': h2"
//@ diag: 38:10 note "in instantiation of template class 'H2<char>' requested here"
//@ diag: 39:9 note "while substituting deduced template arguments into function template 'k' [with T = char]"
//@ diag: 35:56 note "expression evaluates to '1 == 0'"
//@ diag: 39:9 error "no matching function for call to 'k'"
//@ diag: 38:22 note "candidate template ignored: substitution failure [with T = char]"
//@ diag: 40:46 error "static assertion failed due to requirement 'sizeof(int) == 0': h3"
//@ diag: 41:41 note "in instantiation of template class 'H3<int>' requested here"
//@ diag: 42:15 note "in instantiation of function template specialization 'm<int>' requested here"
//@ diag: 40:56 note "expression evaluates to '4 == 0'"
//@ diag: 43:60 error "static assertion failed due to requirement 'sizeof(int) == 0': h6"
//@ diag: 44:46 note "in instantiation of template class 'N::H6<int>' requested here"
//@ diag: 45:15 note "in instantiation of function template specialization 'q<int>' requested here"
//@ diag: 43:70 note "expression evaluates to '4 == 0'"
//@ diag: 46:75 error "static assertion failed due to requirement 'sizeof(int) == 0': i"
//@ diag: 47:47 note "in instantiation of template class 'O<int>::I<int>' requested here"
//@ diag: 48:15 note "in instantiation of function template specialization 'r<int>' requested here"
//@ diag: 46:85 note "expression evaluates to '4 == 0'"
//@ diag: 49:46 error "static assertion failed due to requirement 'sizeof(int) == 0': h4"
//@ diag: 50:9 note "in instantiation of template class 'H4<int>' requested here"
//@ diag: 49:56 note "expression evaluates to '4 == 0'"
//@ diag: 51:46 error "static assertion failed due to requirement 'sizeof(int) == 0': h5"
//@ diag: 52:10 note "in instantiation of template class 'H5<int>' requested here"
//@ diag: 51:56 note "expression evaluates to '4 == 0'"
template <class T> struct H2 { static_assert(sizeof(T) == 0, "h2"); using type = int; };
template <class T> constexpr bool small = sizeof(T) < 2;
template <class T> requires small<T>
typename H2<T>::type k(T);
int f = k('c');
template <class T> struct H3 { static_assert(sizeof(T) == 0, "h3"); using type = int; };
template <class T> void m(T) { typename H3<T>::type x = 0; }
template void m(int);
namespace N { template <class T> struct H6 { static_assert(sizeof(T) == 0, "h6"); using type = int; }; }
template <class T> void q(T) { typename ::N::H6<T>::type y = 0; }
template void q(int);
template <class T> struct O { template <class U> struct I { static_assert(sizeof(U) == 0, "i"); using type = int; }; };
template <class T> void r(T) { typename O<T>::template I<T>::type z = 0; }
template void r(int);
template <class T> struct H4 { static_assert(sizeof(T) == 0, "h4"); };
H4<int> h4;
template <class T> struct H5 { static_assert(sizeof(T) == 0, "h5"); static const int v = 0; };
int v5 = H5<int>::v;
