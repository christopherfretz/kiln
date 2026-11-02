// [dcl.fct]/22: a placeholder `auto` invents a template parameter only
// in the parameter-declaration-clause of a function declaration (the
// function declarator applied directly to the declarator-id) or of a
// lambda. In any other prototype, a parameter of a nested function
// declarator, a function type in a type-id, alias, typedef, variable or
// template parameter, it is ill-formed (clang's "'auto' not allowed in
// function prototype that is not a function declaration", reported at the
// `auto`; the innermost prototype first).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:20 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 29:20 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 30:20 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 31:17 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 32:26 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 33:28 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 34:36 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 35:12 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 36:17 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 37:18 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 38:21 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 40:21 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 41:27 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 43:18 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 44:14 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 45:30 error "'auto' not allowed in function prototype that is not a function declaration"
//@ diag: 45:21 error "'auto' not allowed in function prototype that is not a function declaration"
void f1(void (*cb)(auto));
void f2(auto (*cb)(auto));
void f3(void (*cb)(auto (*)(int)));
void f4(void cb(auto));
auto f5(int) -> void (*)(auto);
void f6(void (*(*cb)(int))(auto));
void f7(auto (*cb)(int) -> int (*)(auto));
void (*f8)(auto);
using F9 = void(auto);
typedef void F10(auto);
template <void (*F)(auto)> struct X11;
struct S12 {
  void m(void (*cb)(auto));
  friend void fr(void (*)(auto));
};
void (*f13(int))(auto);
int f14(void(auto));
void f15(auto (*cb)(auto (*)(auto)));
