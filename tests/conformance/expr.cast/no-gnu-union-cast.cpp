// [expr.cast]/4: C++ has no GNU cast to a union type (GCC and clang accept
// it in C only, clang `CheckCXXCStyleCast`): before C++20 `(U)x` from a
// member's type is a failed static_cast (no constructor of `U` takes it).
// From C++20 the first member is initialized by parenthesized aggregate
// initialization ([dcl.init.general]/16.6.2.2), a later one never.
// WP-1587.
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:7 error "no matching conversion for C-style cast from 'int' to 'U'"
//@ diag: 18:7 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const U' for 1st argument"
//@ diag: 18:7 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'U' for 1st argument"
//@ diag: 18:7 note "candidate constructor (the implicit default constructor) not viable: requires 0 arguments, but 1 was provided"
//@ diag: 20:7 error "no matching conversion for C-style cast from 'double' to 'U'"
//@ diag: 18:7 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'double' to 'const U' for 1st argument"
//@ diag: 18:7 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'double' to 'U' for 1st argument"
//@ diag: 18:7 note "candidate constructor (the implicit default constructor) not viable: requires 0 arguments, but 1 was provided"
union U { int i; double d; };
U a = (U)1;
U b = (U)2.5;
