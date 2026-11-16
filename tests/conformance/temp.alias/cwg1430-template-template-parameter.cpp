// [temp.alias]/2, CWG1430: a pack expansion reaching a non-pack parameter
// of an alias template through a template template parameter is
// ill-formed when the alias is substituted; the error is at the pack
// expansion as written. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ args: -std=c++20
//@ diag: 14:43 error "pack expansion used as argument for non-pack parameter of alias template"
//@ diag: 17:11 note "in instantiation of template class 'quote<one>' requested here"
//@ diag: 16:17 note "template parameter is declared here"
template <class T, class U> struct P {};
template <template <class...> class TT> struct quote {
  template <class... Ts> using apply = TT<Ts...>;
};
template <class T> using one = int;
using t = quote<one>::apply<int>;
// Valid: an alias template whose parameter is a pack, and a class template.
template <class... T> using many = int;
using u = quote<many>::apply<int, char>;
using w = quote<P>::apply<int, char>;
