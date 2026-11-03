// [dcl.init.general]/16, [dcl.init.list]/3: an initializer whose element
// names nothing (clang's ExprError: an undeclared identifier, a qualified
// name not found) is not performed -- clang's ActOnInitializerError -- so
// only the lookup error is reported; a braced list holding one, at any
// depth, fails as a whole, also as a call argument or functional cast.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 35:12 error "no member named 'nom' in 'S'"
//@ diag: 36:16 error "no member named 'nom' in 'S'"
//@ diag: 37:13 error "no member named 'nom' in 'S'"
//@ diag: 37:21 error "no member named 'nom' in 'S'"
//@ diag: 38:10 error "use of undeclared identifier 'nom'"
//@ diag: 39:13 error "no member named 'nom' in 'S'"
//@ diag: 40:16 error "no member named 'nom' in 'S'"
//@ diag: 41:16 error "no member named 'nom' in 'S'"
//@ diag: 42:22 error "no member named 'nom' in 'S'"
//@ diag: 43:19 error "no member named 'nom' in 'S'"
//@ diag: 44:14 error "no member named 'nom' in 'S'"
//@ diag: 45:16 error "no member named 'nom' in 'S'"
//@ diag: 46:24 error "no member named 'nom' in 'S'"
//@ diag: 47:19 error "no member named 'nom' in 'S'"
//@ diag: 48:16 error "no member named 'nom' in 'S'"
//@ diag: 49:17 error "no member named 'nom' in 'S'"
//@ diag: 50:9 error "no member named 'nom' in 'S'"
//@ diag: 52:10 error "no member named 'nom' in 'S'"
//@ diag: 55:5 error "excess elements in scalar initializer"
struct S {};
struct P { int a; };
void g(int);

// An element naming nothing makes the whole initializer invalid: no
// "excess elements", no conversion, no overload resolution follows.
void f() {
  int w(S::nom, 2);
  int w2(2, S::nom);
  int w3(S::nom, S::nom, 3);
  int w4(nom, 2);
  int w5{S::nom, 2};
  int w6 = (S::nom, 2);
  int a1{1, S::nom};
  int a2[1] = {1, S::nom};
  int a3[1] = {S::nom, 2};
  P a4{1, S::nom};
  int a5 = {S::nom, 2};
  int *a6 = new int(S::nom, 2);
  int a7 = int(S::nom, 2);
  (void)int{S::nom, 2};
  int a8{1, {S::nom}};
  g({S::nom, 2});
}
int h(S::nom, 2);

// A valid initializer with too many elements is still diagnosed.
int ok(1, 2);
