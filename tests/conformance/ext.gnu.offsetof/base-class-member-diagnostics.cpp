// offsetof of a member of a virtual base is an error (its offset is not a
// constant), and an ambiguous class member lookup is diagnosed as for any
// member name ([class.member.lookup]/6; clang BuildBuiltinOffsetOf).
//@ args: -Wno-invalid-offsetof
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:41 error "invalid application of 'offsetof' to a field of a virtual base"
//@ diag: 14:41 error "non-static member 'capacity' found in multiple base-class subobjects of type 'B':"
//@ diag: 10:24 note "member found by ambiguous name lookup"
struct B { int a; long capacity; };
struct V : virtual B {};
struct A1 : B {}; struct A2 : B {}; struct M : A1, A2 {};
unsigned long v = __builtin_offsetof(V, capacity);
unsigned long m = __builtin_offsetof(M, capacity);
