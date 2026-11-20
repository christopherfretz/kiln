// __builtin_structured_binding_size(T) of a type with no decomposition
// is "type 'T' cannot be bound" at the keyword, after the error that
// rejects a class (an anonymous member, an incomplete type); an
// inaccessible member is an error at the operand, but the count stands
// (clang 22 GetDecompositionElementCount; WP-1740). Oracle-checked by
// hand with clang++-22 (22.1.8).
//@ expect: compile-fail
//@ requires: clang>=22
//@ std: c++26
//@ strict-diags
//@ diag: 28:19 error "type 'int' cannot be bound"
//@ diag: 29:19 error "type 'U' cannot be bound"
//@ diag: 30:53 error "cannot bind class type 'Q' because it has an anonymous struct member"
//@ diag: 24:12 note "declared here"
//@ diag: 30:19 error "type 'Q' cannot be bound"
//@ diag: 31:53 error "cannot bind private member 'a' of 'P'"
//@ diag: 25:15 note "implicitly declared private here"
//@ diag: 32:53 error "incomplete type 'I' where a complete type is required"
//@ diag: 26:8 note "forward declaration of 'I'"
//@ diag: 32:19 error "type 'I' cannot be bound"
//@ diag: 33:19 error "type trait requires 1 argument; have 2 arguments"
//@ diag: 34:19 error "type 'int[]' cannot be bound"
union U { int a; };
struct Q { struct { int i; }; };
class P { int a; };
struct I;
struct A { int x; };
unsigned long a = __builtin_structured_binding_size(int);
unsigned long b = __builtin_structured_binding_size(U);
unsigned long c = __builtin_structured_binding_size(Q);
unsigned long d = __builtin_structured_binding_size(P);
unsigned long e = __builtin_structured_binding_size(I);
unsigned long f = __builtin_structured_binding_size(A, A);
unsigned long g = __builtin_structured_binding_size(int[]);
