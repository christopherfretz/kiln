// A struct argument that is an lvalue-to-rvalue conversion of an object is
// passed from that object without a copy (C17 6.5.2.2/4 only asks for its
// value; clang `EmitCallArg` `addUncopiedAggregate`): a register-passed
// one is loaded from the object, a `byval` one points at it. Found while
// comparing the AAPCS64 pure scalable type lowering with clang (WP-1739).
//@ expect: ir
//@ xfail(kiln): WP-1739 follow-up, kiln copies every aggregate argument into a temporary before the call (clang passes an lvalue argument uncopied)
struct T { long a, b; };
struct U { long a, b, c; };
void g(struct T);
void gu(struct U);
void h(struct T s) { g(s); }
void hu(struct U s) { gu(s); }
