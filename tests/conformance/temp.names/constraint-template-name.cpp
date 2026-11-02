// [temp.names]/8, [temp.alias]: "constraints not satisfied" for an alias,
// class or variable template names the template bare, however its
// template-id was qualified, as clang does.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:18 error "constraints not satisfied for alias template 'rv' [with T = char]"
//@ diag: 28:24 note "because 'char' does not satisfy 'R'"
//@ diag: 27:31 note "because 'sizeof(char) > 1' (1 > 1) evaluated to false"
//@ diag: 32:7 error "constraints not satisfied for class template 'C' [with T = char]"
//@ diag: 28:52 note "because 'char' does not satisfy 'R'"
//@ diag: 27:31 note "because 'sizeof(char) > 1' (1 > 1) evaluated to false"
//@ diag: 33:16 error "constraints not satisfied for variable template 'v' [with T = char]"
//@ diag: 28:79 note "because 'char' does not satisfy 'R'"
//@ diag: 27:31 note "because 'sizeof(char) > 1' (1 > 1) evaluated to false"
//@ diag: 35:12 error "constraints not satisfied for alias template 'rv' [with T = char]"
//@ diag: 28:24 note "because 'char' does not satisfy 'R'"
//@ diag: 27:31 note "because 'sizeof(char) > 1' (1 > 1) evaluated to false"
//@ diag: 36:19 error "constraints not satisfied for alias template 'rv' [with T = char]"
//@ diag: 37:1 note "in instantiation of template type alias 'W' requested here"
//@ diag: 28:24 note "because 'char' does not satisfy 'R'"
//@ diag: 27:31 note "because 'sizeof(char) > 1' (1 > 1) evaluated to false"
//@ diag: 38:36 error "constraints not satisfied for class template 'C' [with T = char]"
//@ diag: 39:9 note "in instantiation of template class 'Z<char>' requested here"
//@ diag: 28:52 note "because 'char' does not satisfy 'R'"
//@ diag: 27:31 note "because 'sizeof(char) > 1' (1 > 1) evaluated to false"
namespace N {
template<class T> concept R = sizeof(T) > 1;
namespace M { template<R T> using rv = T; template<R T> struct C {}; template<R T> constexpr int v = 0; }
template<class T> struct H {};
}
using A1 = N::M::rv<char>;
N::M::C<char> c1;
int i1 = N::M::v<char>;
using namespace N::M;
using A2 = rv<char>;
template<class T> using W = N::M::rv<T>;
W<char> w;
template<class T> struct Z { N::M::C<T> c; int i = N::M::v<T>; };
Z<char> z;
