// [class.dtor]/9, [dcl.constexpr]/3 (C++20): a defaulted destructor is
// constexpr only if every subobject's destructor is; C++23 drops the
// requirement (P2448R2). So in C++20 `B`'s explicitly defaulted
// destructor cannot be `constexpr` (clang: "defaulted definition of
// destructor cannot be marked constexpr before C++23"), while `C`'s
// (no class subobject) and `D`'s (a subobject with a constexpr
// destructor) can. kiln took every defaulted destructor for constexpr,
// which also made an implicit one defined where only a constexpr one
// is (stmt.if/discarded-statement-special-members-run).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 17:17 error "defaulted definition of destructor cannot be marked constexpr before C++23"
struct N {
  ~N() {}
};
struct B { N n; constexpr ~B() = default; };
struct C { int i; constexpr ~C() = default; };
struct K { constexpr ~K() {} };
struct D : K { K k[2]; constexpr ~D() = default; };
