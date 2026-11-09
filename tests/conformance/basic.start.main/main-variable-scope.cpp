// [basic.start.main]/3: declaring a variable `main` at global scope is
// ill-formed, including by a block-scope `extern` declaration whose target
// scope is the global namespace
// (clang's err_main_global_variable, at the decl-specifiers; WP-1015). A
// variable `main` in a namespace, a block-scope non-extern one, a static
// data member, and a block-scope `extern` in a namespace's function are
// fine. clang 22 rewords the error, so the message is matched by prefix.
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:12 error "main cannot be declared as "
//@ diag: 16:14 error "main cannot be declared as "
void z() { extern int main; }
struct S {
  static int main;
  void v() { extern int main; }
};
namespace n {
int main;
void w() { extern int main; }
}
int f() {
  int main = 0;
  return main;
}
