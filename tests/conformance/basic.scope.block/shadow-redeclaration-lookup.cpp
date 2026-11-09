// [basic.scope.block], [namespace.memdef]: -Wshadow finds the hidden
// entity by clang's redeclaration lookup, which searches the block and
// class scopes and then only the innermost enclosing namespace (with its
// inline and unnamed namespaces): not an outer namespace, not a namespace
// a using-directive nominates there. Through a using-declaration it is the
// entity named. An unnamed namespace prints as `(anonymous)`.
//@ verify
//@ args: -Wshadow

namespace P { int pv; namespace Q { void f() { int pv = 0; (void)pv; } } }
namespace A { int av; typedef int T; } // expected-note 3 {{previous declaration is here}}
namespace B { using namespace A; void g() { int av = 0; (void)av; } }
namespace C {
using A::av; using A::T;
void f() {
  int av = 0; // expected-warning {{declaration shadows a variable in namespace 'A'}}
  (void)av;
  typedef long T; // expected-warning {{declaration shadows a typedef in namespace 'A'}}
}
}
void g() { using A::av; { int av = 0; (void)av; } } // expected-warning {{declaration shadows a variable in namespace 'A'}}

inline namespace I { int iv; } // expected-note {{previous declaration is here}}
void h() { int iv = 0; (void)iv; } // expected-warning {{declaration shadows a variable in namespace 'I'}}
namespace O { int o; inline namespace J { void m() { int o = 0; (void)o; } } }

namespace { int an; } // expected-note {{previous declaration is here}}
void k() { int an = 0; (void)an; } // expected-warning {{declaration shadows a variable in namespace '(anonymous)'}}
int x;
namespace { void k2() { int x = 0; (void)x; } }
namespace N {
namespace { int a; namespace M { int b; } } // expected-note 4 {{previous declaration is here}}
void f() { int a = 0; (void)a; } // expected-warning {{declaration shadows a variable in namespace 'N::(anonymous)'}}
namespace { void g() { int a = 0; (void)a; } } // expected-warning {{declaration shadows a variable in namespace 'N::(anonymous)'}}
namespace { namespace M { void h() { int b = 0; (void)b; } } } // expected-warning {{declaration shadows a variable in namespace 'N::(anonymous namespace)::M'}}
struct S { void m(); };
}
void N::S::m() { int a = 0; (void)a; } // expected-warning {{declaration shadows a variable in namespace 'N::(anonymous)'}}
