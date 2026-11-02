// [module.interface]/3 (C++20, as clang 22): an exported redeclaration of
// an entity with module linkage, and an exported using-declaration naming
// an entity with module or internal linkage, are ill-formed.
//@ expect: compile-fail
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
//@ diag: 16:12 error "cannot export redeclaration 'h' here since the previous declaration has module linkage"
//@ diag: 15:5 note "previous declaration is here"
//@ diag: 18:17 error "using declaration referring to 'i' with module linkage cannot be exported"
//@ diag: 17:19 note "target of using declaration"
//@ diag: 20:16 error "using declaration referring to 'j' with internal linkage cannot be exported"
//@ diag: 19:17 note "target of using declaration"
export module M;
int h();
export int h();
namespace n { int i(); }
export using n::i;
namespace { int j; }
export using ::j;
