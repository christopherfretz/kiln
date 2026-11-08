// [dcl.enum]/2, [basic.scope.hiding]: clang 19 diagnoses a member
// enumeration's enumerator that hides a namespace member or an enclosing
// class's member, and a field from a static or explicit object member
// function; never a base class member (clang 22 warns only on a member of
// the enumeration's own class, see shadow-enumerator-member.cpp).
//@ verify
//@ args: -Wshadow
//@ requires: clang<22

namespace YAML {
enum EMITTER_MANIP { Flow, Block }; // expected-note 2 {{previous declaration is here}}
int sm2; // expected-note {{previous declaration is here}}
struct FlowType {
  enum value { NoType, Flow, Block }; // expected-warning 2 {{declaration shadows a variable in namespace 'YAML'}}
};
struct V {
  int field; // expected-note 5 {{previous declaration is here}}
  static int sm; // expected-note {{previous declaration is here}}
  struct In { enum { field, sm, sm2 }; }; // expected-warning {{declaration shadows a field of 'YAML::V'}} expected-warning {{declaration shadows a static data member of 'YAML::V'}} expected-warning {{declaration shadows a variable in namespace 'YAML'}}
  static void s() { enum { field }; } // expected-warning {{declaration shadows a field of 'YAML::V'}}
  static void s2() { [] { enum { field }; }(); } // expected-warning {{declaration shadows a field of 'YAML::V'}}
  void x(this V) { enum { field }; } // expected-warning {{declaration shadows a field of 'YAML::V'}}
  void y() { struct L { enum { field }; }; } // expected-warning {{declaration shadows a field of 'YAML::V'}}
};
struct Base { int bf; };
struct D : Base { enum { bf }; };
}
