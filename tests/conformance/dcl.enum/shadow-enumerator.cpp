// [dcl.enum]/2, [basic.scope.hiding]: -Wshadow on an enumerator of an
// unscoped enumeration that hides a variable, function or enumerator found
// by redeclaration lookup (clang `ActOnEnumConstant`): the innermost
// namespace and the block scopes inside it, not an outer namespace, not an
// overload set, not a scoped enumeration's enumerator. One clang then
// rejects as a redefinition is still diagnosed first; a template's
// enumerators warn once, at the definition.
//@ verify
//@ args: -Wshadow

namespace N {
enum M { Flow, Block }; // expected-note 2 {{previous declaration is here}} expected-note 2 {{previous definition is here}}
int gv;                 // expected-note {{previous declaration is here}}
void gf();              // expected-note {{previous declaration is here}}
void ov();
void ov(int);
enum Fmt { Plain, Flow2, Block }; // expected-warning {{declaration shadows a variable in namespace 'N'}} expected-error {{redefinition of enumerator 'Block'}}
enum { gv2, gv };       // expected-warning {{declaration shadows a variable in namespace 'N'}} expected-error {{redefinition of 'gv'}} expected-note@-5 {{previous definition is here}}
enum { gf2, gf };       // expected-warning {{declaration shadows a variable in namespace 'N'}} expected-error {{redefinition of 'gf'}} expected-note@-5 {{previous definition is here}}
void h() { enum { ov }; }
enum class Sc { Flow };
namespace In { enum { Flow, gv3 }; }
extern "C++" { enum { Flow3, Flow }; } // expected-warning {{declaration shadows a variable in namespace 'N'}} expected-error {{redefinition of enumerator 'Flow'}}
}

int x; // expected-note {{previous declaration is here}}
template <class T> int tf() { enum { x = sizeof(T) }; return x; } // expected-warning {{declaration shadows a variable in the global namespace}}
int ti = tf<int>() + tf<char>();

int lc; // expected-note {{previous declaration is here}}
void f(int p) { // expected-note {{previous declaration is here}}
  int loc = 0; // expected-note 2 {{previous declaration is here}}
  (void)loc;
  { enum { loc, p }; } // expected-warning 2 {{declaration shadows a local variable}}
  auto l = [] { enum { loc }; }; // expected-warning {{declaration shadows a local variable}}
  (void)l;
  enum { lc }; // expected-warning {{declaration shadows a variable in the global namespace}}
}
