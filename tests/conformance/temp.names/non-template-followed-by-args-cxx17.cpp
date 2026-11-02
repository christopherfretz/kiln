// [temp.names]/3 before C++20: a name is a template-name only when lookup
// finds a template (or, clang's TNK_Undeclared_template, nothing at all).
// A single non-template found by lookup -- function, member function,
// variable, enumerator -- followed by `<>` or by `<` type-id ... `>` is
// diagnosed as clang's checkPotentialAngleBracket does: "'f' does not name
// a template but is followed by template arguments" with a note at the
// declaration; the argument list is skipped and a following call's
// arguments are still checked. A name lookup finds nothing for is an
// undeclared identifier (its arguments parsed as a template-id).
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 38:20 error "'g' does not name a template but is followed by template arguments"
//@ diag: 37:7 note "non-template declaration found by name lookup"
//@ diag: 40:9 error "'f' does not name a template but is followed by template arguments"
//@ diag: 33:5 note "non-template declaration found by name lookup"
//@ diag: 41:9 error "'f' does not name a template but is followed by template arguments"
//@ diag: 33:5 note "non-template declaration found by name lookup"
//@ diag: 42:9 error "'v' does not name a template but is followed by template arguments"
//@ diag: 34:5 note "non-template declaration found by name lookup"
//@ diag: 43:9 error "'e0' does not name a template but is followed by template arguments"
//@ diag: 35:10 note "non-template declaration found by name lookup"
//@ diag: 44:9 error "'f' does not name a template but is followed by template arguments"
//@ diag: 33:5 note "non-template declaration found by name lookup"
//@ diag: 46:9 error "use of undeclared identifier 'k'"
//@ diag: 47:9 error "use of undeclared identifier 'undeclared'"
//@ diag: 49:3 error "'f' does not name a template but is followed by template arguments"
//@ diag: 33:5 note "non-template declaration found by name lookup"
//@ diag: 50:11 error "'f' does not name a template but is followed by template arguments"
//@ diag: 33:5 note "non-template declaration found by name lookup"
//@ diag: 50:18 error "use of undeclared identifier 'not_found'"
//@ diag: 50:31 error "use of undeclared identifier 'also_not_found'"
int f(int);
int v;
enum E { e0 };
struct S {
  int g(int);
  int h() { return g<int>(1); }
};
int a = f<int>(1);
int b = f<>(1);
int c = v<int>(1);
int d = e0<long>(2);
int i = f<int, unknown>(1);
namespace N { struct X {}; int k(X); }
int j = k<int>(N::X());
int m = undeclared<int>(1);
void use(int p) {
  f<int>(p);
  int n = f<int>(not_found) + also_not_found;
}
