// Before C++17 there is no class template argument deduction: a class
// template name used without template arguments, in a functional cast,
// a declaration or a braced cast, is an error that names the template.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -std=c++14
//@ strict-diags
//@ diag: 21:15 error "use of class template 'Wrap' requires template arguments"
//@ diag: 19:27 note "template is declared here"
//@ diag: 22:1 error "use of class template 'Wrap' requires template arguments"
//@ diag: 19:27 note "template is declared here"
//@ diag: 23:10 error "use of class template 'Wrap' requires template arguments"
//@ diag: 19:27 note "template is declared here"
//@ diag: 24:13 error "use of class template 'n::Box' requires template arguments"
//@ diag: 17:27 note "template is declared here"
namespace n {
template <class T> struct Box { Box(T) {} };
}
template <class T> struct Wrap { Wrap(T) {} };

Wrap<int> a = Wrap(1);
Wrap b(2);
auto c = Wrap{3};
auto d = n::Box(4);
