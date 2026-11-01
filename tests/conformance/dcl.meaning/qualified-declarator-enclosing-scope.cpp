//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:7 error "definition or redeclaration of 'a' cannot name the global scope"
//@ diag: 26:10 error "definition or redeclaration of 'x' not allowed inside a function"
//@ diag: 27:9 error "definition or redeclaration of 'a' cannot name the global scope"
//@ diag: 28:11 error "definition or redeclaration of 'h' not allowed inside a function"
//@ diag: 29:10 error "definition or redeclaration of 's' not allowed inside a function"
//@ diag: 31:23 error "cannot define or redeclare 'z' here because namespace 'M2' does not enclose namespace 'M'"
//@ diag: 32:21 error "definition or redeclaration of 'a' cannot name the global scope"
//@ diag: 33:18 error "non-friend class member 'a' cannot have a qualified name"
//@ diag: 36:20 error "extra qualification on member 'f'"
//@ diag: 37:44 error "non-friend class member 'f' cannot have a qualified name"
//@ diag: 39:29 error "no member named 'f' in 'A'"
// [dcl.meaning.general]/2: a qualified declarator-id is declared only in a
// scope enclosing the scope its nested-name-specifier names (clang's
// diagnoseQualifiedDeclaration). `T(::a);` in a block is not a
// redeclaration of the global `a`, the block-scope `int N::x;` does not
// add a second definition, and a member qualified by its own or a nested
// class stays a member of that class. WP-191.
namespace N { extern int x; void h(); namespace I { extern int q; void g(); } }
namespace M { extern int z; }
int a;
struct T { T(); T(int); static int s; };
void f() {
  T(::a);
  int N::x;
  int ::a = 2;
  void N::h();
  int T::s;
}
namespace M2 { int M::z; }
namespace N { int ::a; }
struct C { int ::a; };
int N::x = 1;
namespace N { int I::q = 1; void I::g() {} }
struct X { void X::f(); int y; };
struct A { struct B { void f(); }; void B::f(); };
extern "C++" { namespace N { int s; } }
int use() { X x; x.f(); A{}.f(); return N::s + a; }
