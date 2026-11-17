// [class.mem.general], [class.union.general]/2 (CWG1460): at most one
// variant member of a union has a default member initializer, and the
// members of nested anonymous unions are variant members of the enclosing
// one: initializers in two different nested anonymous unions conflict.
// A named union member is not flattened: its members are not variant
// members of the enclosing union. Found running GCC's C++ testsuite.
//@ std: c++20
//@ expect: compile-fail
//@ diag: 15:46 error "initializing multiple members of union"
//@ diag: 15:23 note "previous initialization is here"
//@ diag: 19:3 error "call to implicitly-deleted default constructor of 'N'"
//@ diag: 17:26 note "default constructor of 'N' is implicitly deleted because variant field 'n' has a non-trivial default constructor"
union Named { int x{5}; char y; };
struct M {
  union { union { int x{5}; }; union { short s{1}; }; };
};
struct N { union { Named n; long long z; }; };
// The default member initializer of `n.x` is not one of N's.
N n;
// Accepted: the initialized member is a variant member of the anonymous union.
struct N2 { union { Named n; long long z{1}; }; };
N2 n2;
