// [namespace.udecl]/11 with [basic.scope.scope]/4: in a class, a
// using-declarator whose target corresponds to a member of the class and
// is another entity conflicts with it, in either order: a class and a
// typedef-name of another type correspond (only a variable, data member,
// enumerator or function is exempt from corresponding with a type), as do
// a data member or member function and an enumerator `using enum`
// introduces. [class.mem.general]/5: an enumerator of a member enumeration
// is a member already, and a using-declarator naming it, or `using enum` of
// that enumeration, declares it a second time. Found running GCC's C++
// testsuite; clang 22 accepts the class/typedef pairs, the enumerators of
// the class's own enumeration and a member declared after `using enum`:
// the text and GCC 16 decide. clang reports the member-after-using
// conflict at the class-key; kiln at the name (no column below).
//@ xfail(clang): clang 19 and 22 accept several of these conflicts
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 43:36 error "target of using declaration conflicts with declaration already in scope"
//@ diag: 43:22 note "target of using declaration"
//@ diag: 43:22 note "conflicting declaration"
//@ diag: 44:38 error "target of using declaration conflicts with declaration already in scope"
//@ diag: 44:22 note "target of using declaration"
//@ diag: 44:22 note "conflicting declaration"
//@ diag: 45:31 error "target of using declaration conflicts with declaration already in scope"
//@ diag: 42:16 note "target of using declaration"
//@ diag: 45:17 note "conflicting declaration"
//@ diag: 46:31 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 42:16 note "target of using declaration"
//@ diag: 46:24 note "using declaration"
//@ diag: 47:32 error "target of using declaration conflicts with declaration already in scope"
//@ diag: 42:19 note "target of using declaration"
//@ diag: 47:18 note "conflicting declaration"
//@ diag: 49:42 error "target of using declaration conflicts with declaration already in scope"
//@ diag: 48:24 note "target of using declaration"
//@ diag: 49:24 note "conflicting declaration"
//@ diag: 50 error "declaration conflicts with target of using declaration already in scope"
//@ diag: 48:24 note "target of using declaration"
//@ diag: 50:26 note "using declaration"
//@ diag: 51:36 error "target of using declaration conflicts with declaration already in scope"
//@ diag: 48:53 note "target of using declaration"
//@ diag: 51:24 note "conflicting declaration"
enum class F { x, y };
struct S1 { enum E { A }; using E::A; };
struct S2 { enum E { A }; using enum E; };
struct S3 { int x; using enum F; };
struct S4 { using enum F; int x; };
struct S5 { void y(); using F::y; };
struct H { typedef int type; struct C {}; typedef C D; };
struct J1 : H { struct type {}; using H::type; };
struct J2 : H { using H::type; struct type {}; };
struct J3 : H { struct D; using H::D; };
