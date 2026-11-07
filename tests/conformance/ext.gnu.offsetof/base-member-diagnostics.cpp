// __builtin_offsetof into a base class: an ambiguous member is reported
// as for a member access, and a member of a virtual base has no fixed
// offset (clang's BuildBuiltinOffsetOf).
//@ verify
//@ args: -Wno-invalid-offsetof

struct V { int v; };
struct L : virtual V { int l; };
struct R : virtual V { int r; };
struct J : L, R { int j; };

struct X { int x; }; // expected-note {{member found by ambiguous name lookup}}
struct P : X {};
struct Q : X {};
struct M : P, Q {};

unsigned long a = __builtin_offsetof(J, v); // expected-error {{invalid application of 'offsetof' to a field of a virtual base}}
unsigned long b = __builtin_offsetof(L, v); // expected-error {{invalid application of 'offsetof' to a field of a virtual base}}
unsigned long c = __builtin_offsetof(J, r);
unsigned long d = __builtin_offsetof(M, x); // expected-error {{non-static member 'x' found in multiple base-class subobjects of type 'X':}}
