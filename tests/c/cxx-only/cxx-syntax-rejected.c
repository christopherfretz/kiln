// C17 6.4.1: C++ keywords are ordinary identifiers in C; C++-only constructs
// (references, templates, namespaces, ::, member functions, default
// arguments, new/delete, class) are errors, never silently accepted.
//@ xfail(kiln): WP-1276 follow-up, the C parser still parses & declarators, template<>, ::, class and lambdas as C++ (errors, wrong wording and recovery; clang treats them as plain C tokens)
//@ expect: compile-fail
//@ diag: 15:5 error "expected identifier or '('"
//@ diag: 16:1 error "unknown type name 'template'"
//@ diag: 17:1 error "unknown type name 'namespace'"
//@ diag: 18:9 error "expected expression"
//@ diag: 19:17 error "field 'f' declared as a function"
//@ diag: 20:14 error "C does not support default arguments"
//@ diag: 21:10 error "use of undeclared identifier 'new'"
//@ diag: 22:1 error "unknown type name 'class'"
//@ diag: 23:22 error "expected expression"
int &r;
template <class T> T id(T);
namespace N { int x; }
int y = ::y;
struct M { void f(); };
void g(int a = 1);
int *p = new int;
class C { int z; };
void h(void) { (void)[](){}; }
