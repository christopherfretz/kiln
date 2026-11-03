// [temp.names]/1, [temp.arg.general]: a class or alias template named
// without a template argument list as a nested-name-specifier component
// (`N::Y::v`, `X::type g;`) is "use of class template 'N::Y' requires
// template arguments" at the template name, printed with the specifier as
// written ('::N::Y', 'P<int>::J'), with the "template is declared here" note
// at the template (an alias template's at its `using`), as clang's
// diagnoseMissingTemplateArguments.
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 41:12 error "use of class template 'N::Y' requires template arguments"
//@ diag: 36:27 note "template is declared here"
//@ diag: 42:12 error "use of alias template 'N::A' requires template arguments"
//@ diag: 37:20 note "template is declared here"
//@ diag: 43:9 error "use of alias template 'B' requires template arguments"
//@ diag: 39:20 note "template is declared here"
//@ diag: 44:14 error "use of class template '::N::Y' requires template arguments"
//@ diag: 36:27 note "template is declared here"
//@ diag: 45:15 error "use of class template 'N::Y' requires template arguments"
//@ diag: 36:27 note "template is declared here"
//@ diag: 45:25 error "use of class template 'X' requires template arguments"
//@ diag: 40:27 note "template is declared here"
//@ diag: 46:9 error "use of class template 'X' requires template arguments"
//@ diag: 40:27 note "template is declared here"
//@ diag: 47:1 error "use of class template 'X' requires template arguments"
//@ diag: 40:27 note "template is declared here"
//@ diag: 48:4 error "use of class template 'N::Y' requires template arguments"
//@ diag: 36:27 note "template is declared here"
//@ diag: 49:12 error "use of class template 'X' requires template arguments"
//@ diag: 40:27 note "template is declared here"
//@ diag: 51:12 error "use of class template 'O::I' requires template arguments"
//@ diag: 50:38 note "template is declared here"
//@ diag: 53:17 error "use of class template 'P<int>::J' requires template arguments"
//@ diag: 52:57 note "template is declared here"
namespace N {
template <class T> struct Y { static int v; struct type {}; };
template <class T> using A = Y<T>;
}
template <class T> using B = N::Y<T>;
template <class T> struct X { static int v; };
int a = N::Y::v;
int b = N::A::v;
int c = B::v;
int d = ::N::Y::v;
void f() { N::Y::v = 1; X::v = 2; }
int e = X::v;
X::type g;
N::Y::type h;
void k() { X::type i; }
struct O { template <class U> struct I { static int v; }; };
int l = O::I::v;
template <class T> struct P { template <class U> struct J { static int v; }; };
int m = P<int>::J::v;
