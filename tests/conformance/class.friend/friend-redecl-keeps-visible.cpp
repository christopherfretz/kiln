// [namespace.memdef]/3, [dcl.meaning.general], [dcl.fct.default]/4,
// [dcl.attr.deprecated], [dcl.attr.nodiscard]: a friend declaration that
// redeclares a function already visible in the enclosing namespace keeps
// it visible, with the default arguments and attributes of the earlier
// declaration; a friend declaration does not replace what ordinary lookup
// finds.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 30:6 warning "'dep' is deprecated"
//@ diag: 16:3 note "'dep' has been explicitly marked deprecated here"
//@ diag: 31:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 35:3 error "use of undeclared identifier 'hidden'"
namespace N {
[[deprecated]] void dep();
[[nodiscard]] int nd();
int def(int = 4);
template <class T> int tdef(T, int = 5);
struct S {
  friend void dep();
  friend int nd();
  friend int def(int);
  template <class T> friend int tdef(T, int);
  friend int hidden(int x) { return x; }
  friend int hidden2(S) { return 2; }
};
} // namespace N
void use() {
  N::dep();
  N::nd();
  int a = N::def();
  int b = N::tdef(1);
  int c = hidden2(N::S{});
  hidden();
  (void)(a + b + c);
}
