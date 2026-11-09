// [dcl.inline]/6: a function first declared inline after a non-inline
// definition is ill-formed (clang err_inline_decl_follows_def, at the
// name, with a note at the definition): also for friends, function
// templates, qualified and block-scope redeclarations, and a non-dependent
// friend of a class template at its definition (WP-1042).
//@ expect: compile-fail
//@ strict-diags
//@ std: c++20
//@ diag: 27:13 error "inline declaration of 'a' follows non-inline definition"
//@ diag: 26:6 note "previous definition is here"
//@ diag: 29:31 error "inline declaration of 't' follows non-inline definition"
//@ diag: 28:24 note "previous definition is here"
//@ diag: 31:16 error "inline declaration of 'd' follows non-inline definition"
//@ diag: 30:20 note "previous definition is here"
//@ diag: 33:13 error "inline declaration of 'e' follows non-inline definition"
//@ diag: 32:6 note "previous definition is here"
//@ diag: 35:13 error "inline declaration of 'g' follows non-inline definition"
//@ diag: 34:17 note "previous definition is here"
//@ diag: 38:22 error "inline declaration of 'f1' follows non-inline definition"
//@ diag: 36:6 note "previous definition is here"
//@ diag: 42:22 error "inline declaration of 'f2' follows non-inline definition"
//@ diag: 40:6 note "previous definition is here"
//@ diag: 45:14 error "inline declaration of 'u' not allowed in block scope"
//@ diag: 45:26 error "inline declaration of 'u' follows non-inline definition"
//@ diag: 44:6 note "previous definition is here"
void a() {}
inline void a();
template<class T> void t() {}
template<class T> inline void t();
namespace N { void d() {} }
inline void N::d();
void e() {}
inline void e() {}
extern "C" void g() {}
inline void g();
void f1() {}
struct C1 {
  friend inline void f1();
};
void f2() {}
template<class T> struct C2 {
  friend inline void f2() {}
};
void u() {}
void use() { inline void u(); }
