// [dcl.enum]/2, [basic.scope.hiding]: clang 22 diagnoses a member
// enumeration's enumerator only when it hides a member of the
// enumeration's own class, and a field not from a static or explicit
// object member function (clang 19 also warns on the rest, see
// shadow-enumerator-member-19.cpp).
//@ verify
//@ args: -Wshadow
//@ requires: clang>=22

namespace YAML {
enum EMITTER_MANIP { Flow, Block };
int sm2;
struct FlowType {
  enum value { NoType, Flow, Block };
};
struct V {
  int field; // expected-note 2 {{previous declaration is here}}
  static int sm;
  struct In { enum { field, sm, sm2 }; };
  static void s() { enum { field }; }
  static void s2() { [] { enum { field }; }(); }
  void x(this V) { enum { field }; }
  void y() { struct L { enum { field }; }; }
  void m() { [] { enum { field }; }(); } // expected-warning {{declaration shadows a field of 'YAML::V'}}
  V() { enum { field }; } // expected-warning {{declaration shadows a field of 'YAML::V'}}
};
template <class T> struct Tm {
  int tf; // expected-note {{previous declaration is here}} expected-note {{previous definition is here}}
  enum { tf2 = sizeof(T), tf }; // expected-warning {{declaration shadows a field of 'YAML::Tm<T>'}} expected-error {{redefinition of 'tf'}}
};
}
