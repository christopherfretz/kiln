// [dcl.constexpr]/6 (C++20): temporaries in the never-constant walk
// (`isPotentialConstantExpr`). Destroying an object of a class with a
// non-trivial destructor always fails (the destructor call is refused), and
// once a side effect has been noted so does destroying a discarded class
// prvalue or a block's objects. A failure at the end of an `ExprWithCleanups`
// (a temporary bound by a member access or discarded by a cast) only notes a
// side effect and the walk goes on; one at the end of the statement (a
// discarded class prvalue, a condition) or of a block (a lifetime-extended
// temporary) stops the walk quietly (WP-1133).
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wno-unused-value -Wno-c++23-extensions -ferror-limit=0
//@ strict-diags
//@ diag: 36:15 error "constexpr function never produces a constant expression"
//@ diag: 36:68 note "control flows through the definition of a static variable"
//@ diag: 37:15 error "constexpr function never produces a constant expression"
//@ diag: 37:58 note "control flows through the definition of a static variable"
//@ diag: 38:15 error "constexpr function never produces a constant expression"
//@ diag: 38:46 note "control flows through the definition of a static variable"
//@ diag: 39:15 error "constexpr function never produces a constant expression"
//@ diag: 39:61 note "control flows through the definition of a static variable"
//@ diag: 40:15 error "constexpr function never produces a constant expression"
//@ diag: 40:68 note "control flows through the definition of a static variable"
//@ diag: 41:15 error "constexpr function never produces a constant expression"
//@ diag: 41:74 note "control flows through the definition of a static variable"
//@ diag: 42:15 error "constexpr function never produces a constant expression"
//@ diag: 42:63 note "control flows through the definition of a static variable"
//@ diag: 43:15 error "constexpr function never produces a constant expression"
//@ diag: 43:66 note "control flows through the definition of a static variable"

struct S { int v; constexpr int m() const { return v; } };
struct D { int v; constexpr ~D() {} };
constexpr S retS(int);

// Reported.
constexpr int after_side_effect(int x) { x + 1; S{1}.v; static int i = 0; return i; }
constexpr int member_of_temp(int x) { S{1}.v; static int i = 0; return i; }
constexpr int temp(int x) { S{1}; static int i = 0; return i; }
constexpr int destructor_member(int x) { D{1}.v; static int i = 0; return i; }
constexpr int void_destructor_temp(int x) { (void)D{1}; static int i = 0; return i; }
constexpr int init_destructor_member(int x) { int y = D{1}.v; static int i = 0; return i; }
constexpr int extended(int x) { const S& r = S{1}; static int i = 0; return i; }
constexpr int condition_temp(int x) { if (S{1}.v) { } static int i = 0; return i; }

// Silent.
constexpr int void_call_after_side_effect(int x) { x + 1; (void)retS(1); static int i = 0; return i; }
constexpr int void_temp_after_side_effect(int x) { x + 1; (void)S{1}; static int i = 0; return i; }
constexpr int temp_after_side_effect(int x) { x + 1; S{1}; static int i = 0; return i; }
constexpr int destructor_temp(int x) { D{1}; static int i = 0; return i; }
constexpr int unknown_temp(int x) { S{x}; static int i = 0; return i; }
constexpr int void_destructor_then_block(int x) { (void)D{1}; { int y = 0; } static int i = 0; return i; }
constexpr int init_destructor_then_block(int x) { int y = D{1}.v; { int z = 0; } static int i = 0; return i; }
constexpr int extended_after_side_effect(int x) { x + 1; { const S& r = S{1}; } static int i = 0; return i; }
constexpr int extended_destructor(int x) { { const D& r = D{1}; } static int i = 0; return i; }
constexpr int condition_temp_after_side_effect(int x) { x + 1; if (S{1}.v) { } static int i = 0; return i; }
constexpr int condition_destructor(int x) { if (D{1}.v) { } static int i = 0; return i; }
