// [dcl.enum]/2 with [dcl.type.general]/3: an enumerator's initializer may
// not define a type (clang's err_type_defined_in_enum). clang 22 prints the
// tag without an unscoped enumeration's scope, which is transparent, and
// keeps a scoped one (clang 19: type-defined-in-enumeration-19.cpp).
//@ std: c++20
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:28 error "'S1' cannot be defined in an enumeration"
//@ diag: 13:60 error "'E2' cannot be defined in an enumeration"
//@ diag: 14:47 error "'N::F::U' cannot be defined in an enumeration"
//@ diag: 15:40 error "'C::S3' cannot be defined in an enumeration"
enum E { a = sizeof(struct S1 { int x; }), b = sizeof(enum E2 { q }) };
namespace N { enum class F { c = sizeof(union U { int y; }) }; }
struct C { enum G { d = alignof(struct S3 { int z; }) }; };
