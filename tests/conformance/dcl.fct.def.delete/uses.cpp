// [dcl.fct.def.delete]/2: a program that refers to a deleted function, other
// than to declare it, is ill-formed: calls, address-of, member calls, qualified
// names and unevaluated operands; [dcl.init.general]/7 a deleted default ctor.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 41:14 error "attempt to use a deleted function"
//@ diag: 39:8 note "'m' has been explicitly marked deleted here"
//@ diag: 41:25 error "attempt to use a deleted function"
//@ diag: 39:8 note "'m' has been explicitly marked deleted here"
//@ diag: 45:3 error "call to deleted function 'f'"
//@ diag: 36:6 note "candidate function has been explicitly deleted"
//@ diag: 46:3 error "call to deleted function 'g': no ints"
//@ diag: 37:6 note "candidate function has been explicitly deleted"
//@ diag: 47:13 error "attempt to use a deleted function"
//@ diag: 36:6 note "'f' has been explicitly marked deleted here"
//@ diag: 48:17 error "attempt to use a deleted function"
//@ diag: 36:6 note "'f' has been explicitly marked deleted here"
//@ diag: 49:4 error "attempt to use a deleted function"
//@ diag: 36:6 note "'f' has been explicitly marked deleted here"
//@ diag: 51:5 error "attempt to use a deleted function"
//@ diag: 39:8 note "'m' has been explicitly marked deleted here"
//@ diag: 52:6 error "attempt to use a deleted function"
//@ diag: 39:8 note "'m' has been explicitly marked deleted here"
//@ diag: 53:6 error "attempt to use a deleted function"
//@ diag: 40:15 note "'s' has been explicitly marked deleted here"
//@ diag: 54:6 error "attempt to use a deleted function"
//@ diag: 43:20 note "'k' has been explicitly marked deleted here"
//@ diag: 56:3 error "call to deleted function 'k'"
//@ diag: 43:20 note "candidate function has been explicitly deleted"
//@ diag: 57:12 error "call to deleted function 'f'"
//@ diag: 36:6 note "candidate function has been explicitly deleted"
//@ diag: 60:4 error "call to deleted constructor of 'NC'"
//@ diag: 59:13 note "'NC' has been explicitly marked deleted here"
//@ diag: 61:4 error "call to deleted constructor of 'NC[2]'"
//@ diag: 59:13 note "'NC' has been explicitly marked deleted here"
void f() = delete;
void g(int) = delete("no ints");
struct S {
  void m() = delete;
  static void s() = delete;
  void h() { m(); this->m(); }
};
namespace N { void k() = delete; }
void use() {
  f();
  g(1);
  auto p = &f;
  void (*q)() = f;
  (f)();
  S x;
  x.m();
  (x.m)();
  S::s();
  N::k();
  using N::k;
  k();
  decltype(f()) *r;
}
struct NC { NC() = delete; };
NC nc;
NC nca[2];
void h() = delete;
void h();
