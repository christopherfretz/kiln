// [basic.scope.block], [basic.lookup]: -Wshadow names the kind and the
// context of the declaration a local variable, parameter, typedef or
// structured binding hides, with a note at the previous declaration.
// Members inherited from a base class, extern locals, the placeholder `_`
// and template instantiations do not warn.
//@ verify
//@ args: -Wshadow

int g; // expected-note {{previous declaration is here}} expected-note {{previous declaration is here}} expected-note {{previous declaration is here}}
typedef int GT; // expected-note {{previous declaration is here}}
namespace N { int nv; struct NS { int f; static int sm; void m(); }; } // expected-note {{previous declaration is here}} expected-note {{previous declaration is here}}
extern "C" { int cv; } // expected-note {{previous declaration is here}} expected-note {{previous declaration is here}}
struct S { int f; static int sm; void m(int f); void k(); static void st(); }; // expected-note {{previous declaration is here}} expected-note {{previous declaration is here}} expected-note {{previous declaration is here}}
void S::m(int f) { int sm = 0; (void)sm; } // expected-warning {{declaration shadows a field of 'S'}} expected-warning {{declaration shadows a static data member of 'S'}}
void S::k() { int f = 0; typedef int sm; (void)f; } // expected-warning {{declaration shadows a field of 'S'}}
void S::st() { int f = 0; (void)f; }
void N::NS::m() { int nv = 0; int f = 1; int g = 2; (void)nv; (void)f; (void)g; } // expected-warning {{declaration shadows a variable in namespace 'N'}} expected-warning {{declaration shadows a field of 'N::NS'}} expected-warning {{declaration shadows a variable in the global namespace}}
struct P { int a, b; };
void h(int g) { // expected-warning {{declaration shadows a variable in the global namespace}} expected-note {{previous declaration is here}} expected-note {{previous declaration is here}} expected-note {{previous declaration is here}} expected-note {{previous declaration is here}}
  int cv = 0; // expected-warning {{declaration shadows a variable in the global namespace}}
  typedef long GT; // expected-warning {{declaration shadows a typedef in the global namespace}} expected-note {{previous declaration is here}}
  using U = int; // expected-note {{previous declaration is here}}
  { using U = long; typedef int GT; int g = 0; (void)g; } // expected-warning {{declaration shadows a type alias in function 'h'}} expected-warning {{declaration shadows a typedef in function 'h'}} expected-warning {{declaration shadows a local variable}}
  auto [a, b] = P{1, 2}; // expected-note {{previous declaration is here}}
  { int a = 0; auto [g, x] = P{1, 2}; (void)a; (void)x; } // expected-warning {{declaration shadows a structured binding}} expected-warning {{declaration shadows a local variable}}
  (void)cv;
  extern int g2;
  for (int g = 0; g < 1; ++g) {} // expected-warning {{declaration shadows a local variable}}
  if (int g = 1) { (void)g; } // expected-warning {{declaration shadows a local variable}}
  static int sv = 0; { static int sv = 1; (void)sv; } // expected-warning {{declaration shadows a local variable}} expected-note {{previous declaration is here}}
  struct L { void mm() { int cv = 0; (void)cv; } };
}
void decl_only(int g);
template <class T> void tt(T g) { int cv = 0; (void)cv; (void)g; } // expected-warning {{declaration shadows a variable in the global namespace}} expected-warning {{declaration shadows a variable in the global namespace}}
template void tt(int);
template void tt(long);
int _ = 0;
void ph() { int _ = 1; }
void ovl(int); void ovl(long);
void use_ovl() { int ovl = 0; (void)ovl; }
struct A { int a; static int sa; typedef int T; };
struct B : A { void m(int a) { int sa = 0; typedef int T; (void)sa; (void)a; } };
struct Outer { int o; struct In { void m() { int o = 0; (void)o; } }; }; // expected-warning {{declaration shadows a field of 'Outer'}} expected-note {{previous declaration is here}}
