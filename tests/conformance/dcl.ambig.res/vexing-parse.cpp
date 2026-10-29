// -Wvexing-parse: a declaration that looks like an object with a
// parenthesized initializer is a function declaration.
//@ expect: compile-fail
//@ diag: 16:8 warning "empty parentheses interpreted as a function declaration"
//@ diag: 16:8 note "replace parentheses with an initializer to declare a variable"
//@ diag: 17:6 warning "parentheses were disambiguated as a function declaration"
//@ diag: 17:7 note "add a pair of parentheses to declare a variable"
//@ diag: 18:6 warning "empty parentheses interpreted as a function declaration"
//@ diag: 18:6 note "remove parentheses to declare a variable"
//@ diag: 19:8 warning "parentheses were disambiguated as a function declaration"
//@ diag: 19:9 note "add a pair of parentheses to declare a variable"
//@ diag: 21:5 error "non-object type 'int ()' is not assignable"
typedef int T;
struct S {};
void f(double d) {
  int a();
  T b(int(d));
  S s();
  int c(T());
  int e((int(d)));
  a = 1;
}
