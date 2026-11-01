// [dcl.ambig.res], [dcl.name]: -Wredundant-parens (clang's
// warnAboutRedundantParens) also covers parameter declarators, exception
// declarations and abstract declarators in type-ids: the outermost
// declarator operator is a pair of grouping parentheses. Not when a
// function or array chunk is outermost, and a template's type-ids warn
// once, at the definition. WP-235.
//@ expect: compile-ok
//@ args: -Wredundant-parens
//@ strict-diags
// Verified against clang 19 (WP-235).
//@ diag: 36:18 warning "redundant parentheses surrounding declarator"
//@ diag: 37:13 warning "redundant parentheses surrounding declarator"
//@ diag: 38:13 warning "redundant parentheses surrounding declarator"
//@ diag: 40:13 warning "redundant parentheses surrounding declarator"
//@ diag: 42:13 warning "redundant parentheses surrounding declarator"
//@ diag: 44:13 warning "redundant parentheses surrounding declarator"
//@ diag: 45:13 warning "redundant parentheses surrounding declarator"
//@ diag: 46:14 warning "redundant parentheses surrounding declarator"
//@ diag: 47:14 warning "redundant parentheses surrounding declarator"
//@ diag: 48:20 warning "redundant parentheses surrounding declarator"
//@ diag: 49:15 warning "redundant parentheses surrounding declarator"
//@ diag: 50:15 warning "redundant parentheses surrounding declarator"
//@ diag: 51:20 warning "redundant parentheses surrounding declarator"
//@ diag: 53:7 warning "redundant parentheses surrounding declarator"
//@ diag: 54:14 warning "redundant parentheses surrounding declarator"
//@ diag: 55:27 warning "redundant parentheses surrounding declarator"
//@ diag: 55:51 warning "redundant parentheses surrounding declarator"
//@ diag: 57:17 warning "redundant parentheses surrounding declarator"
//@ diag: 58:42 warning "redundant parentheses surrounding declarator"
//@ diag: 59:15 warning "redundant parentheses surrounding declarator"
//@ diag: 59:24 warning "redundant parentheses surrounding declarator"
//@ diag: 60:14 warning "redundant parentheses surrounding declarator"
//@ diag: 62:25 warning "redundant parentheses surrounding declarator"
//@ diag: 63:21 warning "redundant parentheses surrounding declarator"
//@ diag: 64:19 warning "redundant parentheses surrounding declarator"
struct S { S(int (x)); operator int(); };
void f1(int (x));
void f2(int (*p));
void f3(int (*)(int));
void f4(int (*));
void f5(int ());
void f6(int (&r));
void f7(int (x)[2]);
void f8(int ((z)));
void f9(int (*(p)));
void f10(int (&&r));
void f11(int (S::*pm));
int a = sizeof(int (*));
using P = int (*);
using F = int (());
void *q = new (int (*));
template<class T> struct W {};
W<int (*)> w;
template<int (N)> struct X {};
template<class T> int g(T (*p)) { return sizeof(T (*)); }
int u = g<int>(nullptr) + g<long>(nullptr);
auto h() -> int (*);
template<class T> concept C = requires(T (t)) { t; };
int (*fp)(int (b), int (*));
int y = (int (*))0 != 0;
void k() {
  (void)static_cast<int (*)>(nullptr);
  try {} catch (int (e)) {}
  auto l = [](int (v)) {};
  (void)l;
}
