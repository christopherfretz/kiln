// A dependent __builtin_offsetof is diagnosed when substituted: a
// non-class type and a missing member, each with the instantiation note
// (clang 19 BuildBuiltinOffsetOf under TransformOffsetOfExpr; WP-307).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:60 error "offsetof requires struct, union, or class type, 'int' invalid"
//@ diag: 13:19 note "in instantiation of function template specialization 'off_b<int>' requested here"
//@ diag: 12:60 error "no member named 'zz' in 'P'"
//@ diag: 14:19 note "in instantiation of function template specialization 'off_z<P>' requested here"
struct P { int a; double b; int arr[4]; };
template<class T> constexpr unsigned long off_b() { return __builtin_offsetof(T, b); }
template<class T> constexpr unsigned long off_z() { return __builtin_offsetof(T, zz); }
unsigned long a = off_b<int>();
unsigned long b = off_z<P>();
