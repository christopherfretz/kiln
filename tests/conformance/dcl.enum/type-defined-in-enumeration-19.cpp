// [dcl.enum]/2 with [dcl.type.general]/3: an enumerator's initializer may
// not define a type. clang 19 scopes the tag by its enumeration, scoped or
// not (clang 22: type-defined-in-enumeration.cpp).
//@ std: c++20
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:28 error "'E::S1' cannot be defined in an enumeration"
//@ diag: 12:60 error "'E::E2' cannot be defined in an enumeration"
//@ diag: 13:47 error "'N::F::U' cannot be defined in an enumeration"
//@ diag: 14:40 error "'C::G::S3' cannot be defined in an enumeration"
enum E { a = sizeof(struct S1 { int x; }), b = sizeof(enum E2 { q }) };
namespace N { enum class F { c = sizeof(union U { int y; }) }; }
struct C { enum G { d = alignof(struct S3 { int z; }) }; };
