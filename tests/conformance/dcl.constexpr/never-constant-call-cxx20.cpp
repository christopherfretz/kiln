// [dcl.constexpr]/6 (C++20): clang's never-constant check
// (`isPotentialConstantExpr`) refuses every nested call quietly. A discarded
// call, or any other discarded expression that cannot be evaluated, only
// notes a side effect (`EvaluateIgnoredValue`), and the walk goes on to a
// later static variable. A failed variable initializer does the same. Once a
// side effect is noted, destroying a block that holds a local fails quietly,
// so the walk stops there; an `if` with an unknown condition stops it too
// (WP-1133).
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wno-c++23-extensions -Wno-unused-value -Wno-return-type -ferror-limit=0
//@ strict-diags
//@ diag: 66:16 error "constexpr function never produces a constant expression"
//@ diag: 66:25 note "subexpression not valid in a constant expression"
//@ diag: 76:15 error "constexpr function never produces a constant expression"
//@ diag: 76:58 note "control flows through the definition of a static variable"
//@ diag: 77:15 error "constexpr function never produces a constant expression"
//@ diag: 77:56 note "control flows through the definition of a static variable"
//@ diag: 78:15 error "constexpr function never produces a constant expression"
//@ diag: 78:58 note "control flows through the definition of a static variable"
//@ diag: 79:15 error "constexpr function never produces a constant expression"
//@ diag: 79:45 note "non-constexpr function 'nonc' cannot be used in a constant expression"
//@ diag: 67:6 note "declared here"
//@ diag: 80:15 error "constexpr function never produces a constant expression"
//@ diag: 80:67 note "control flows through the definition of a static variable"
//@ diag: 81:15 error "constexpr function never produces a constant expression"
//@ diag: 81:65 note "control flows through the definition of a static variable"
//@ diag: 82:15 error "constexpr function never produces a constant expression"
//@ diag: 82:56 note "control flows through the definition of a static variable"
//@ diag: 83:15 error "constexpr function never produces a constant expression"
//@ diag: 83:61 note "control flows through the definition of a static variable"
//@ diag: 84:15 error "constexpr function never produces a constant expression"
//@ diag: 84:61 note "control flows through the definition of a static variable"
//@ diag: 85:15 error "constexpr function never produces a constant expression"
//@ diag: 85:62 note "control flows through the definition of a static variable"
//@ diag: 86:15 error "constexpr function never produces a constant expression"
//@ diag: 86:64 note "control flows through the definition of a static variable"
//@ diag: 87:15 error "constexpr function never produces a constant expression"
//@ diag: 87:72 note "control flows through the definition of a static variable"
//@ diag: 88:15 error "constexpr function never produces a constant expression"
//@ diag: 88:53 note "control flows through the definition of a static variable"
//@ diag: 89:15 error "constexpr function never produces a constant expression"
//@ diag: 89:60 note "control flows through the definition of a static variable"
//@ diag: 90:15 error "constexpr function never produces a constant expression"
//@ diag: 90:73 note "control flows through the definition of a static variable"
//@ diag: 91:15 error "constexpr function never produces a constant expression"
//@ diag: 91:56 note "control flows through the definition of a static variable"
//@ diag: 92:15 error "constexpr function never produces a constant expression"
//@ diag: 92:63 note "control flows through the definition of a static variable"
//@ diag: 93:15 error "constexpr function never produces a constant expression"
//@ diag: 93:72 note "control flows through the definition of a static variable"
//@ diag: 94:15 error "constexpr function never produces a constant expression"
//@ diag: 94:49 note "control flows through the definition of a static variable"
//@ diag: 95:15 error "constexpr function never produces a constant expression"
//@ diag: 95:75 note "control flows through the definition of a static variable"
//@ diag: 96:15 error "constexpr function never produces a constant expression"
//@ diag: 96:60 note "control flows through the definition of a static variable"
//@ diag: 97:15 error "constexpr function never produces a constant expression"
//@ diag: 97:66 note "control flows through the definition of a static variable"
//@ diag: 98:15 error "constexpr function never produces a constant expression"
//@ diag: 98:68 note "control flows through the definition of a static variable"
//@ diag: 99:15 error "constexpr function never produces a constant expression"
//@ diag: 99:75 note "control flows through the definition of a static variable"

constexpr void callee(int) {}
constexpr void nope() { throw 1; } // never constant itself
void nonc();
constexpr int g(int);
struct S { int v; constexpr int m() const { return v; } constexpr int m2(int) const { return 0; } };
struct D { int v; constexpr ~D() {} };
constexpr S retS(int);
constexpr D retD(int);
constexpr void takeS(S);

// Reported: the walk goes past the call or initializer to the static.
constexpr int unknown_arg(int x) { callee(x); static int i = 0; return i; }
constexpr int known_arg(int x) { callee(1); static int i = 0; return i; }
constexpr int failing_callee(int x) { nope(); static int i = 0; return i; }
constexpr int non_constexpr_callee(int x) { nonc(); static int i = 0; return i; }
constexpr int init_unknown_call(int x) { int y = g(x); static int i = 0; return i; }
constexpr int init_known_call(int x) { int y = g(1); static int i = 0; return i; }
constexpr int nested(int x) { callee(g(x)); static int i = 0; return i; }
constexpr int assign_call(int x) { x = g(x) + 1; static int i = 0; return i; }
constexpr int member_call(S s, int x) { s.m2(x); static int i = 0; return i; }
constexpr int temp_member_call(int x) { S{x}.m(); static int i = 0; return i; }
constexpr int record_argument(int x) { takeS(S{x}); static int i = 0; return i; }
constexpr int bind_reference(int x) { const S& r = retS(x); static int i = 0; return i; }
constexpr int arithmetic(int x) { x + 1; static int i = 0; return i; }
constexpr int unknown_local(int x) { int y = x; static int i = 0; return i; }
constexpr int local_then_call(int x) { int y = 0; callee(x); static int i = 0; return i; }
constexpr int record_local(int x) { S s{x}; static int i = 0; return i; }
constexpr int comma(int x) { callee(x), callee(x); static int i = 0; return i; }
constexpr int two_declarators(int x) { int y = g(x), z = 0; static int i = 0; return i; }
constexpr int assign(int x) { x = 1; static int i = 0; return i; }
constexpr int block_before_side_effect(int x) { { int y = 0; } static int i = 0; return i; }
constexpr int destructor_local(int x) { D d{x}; static int i = 0; return i; }
constexpr int destructor_local_known(int x) { D d{1}; static int i = 0; return i; }
constexpr int known_temp_member_call(int x) { S{1}.m(); static int i = 0; return i; }
constexpr int temp_member_argument(int x) { callee(retS(x).v); static int i = 0; return i; }

// Silent: the walk stops quietly. An unknown `if` condition stops it; so
// does destroying a block's locals or a discarded record temporary once a
// side effect is noted, and destroying a temporary with a destructor.
constexpr int returns_record(int x) { retS(x); static int i = 0; return i; }
constexpr int call_then_return(int x) { callee(x); return 1; }
constexpr int unknown_if(int x) { if (g(x)) return 1; static int i = 0; return i; }
constexpr int block_after_call(int x) { callee(x); { int y = 0; } static int i = 0; return i; }
constexpr int block_after_arithmetic(int x) { x + 1; { int y = 0; } static int i = 0; return i; }
constexpr int block_after_known_call(int x) { callee(1); { int y = 0; } static int i = 0; return i; }
constexpr int block_after_unknown_init(int x) { int y = g(x); { int z = 0; } static int i = 0; return i; }
constexpr int block_with_unknown_init(int x) { { int y = g(x); } static int i = 0; return i; }
constexpr int block_after_assign(int x) { x = 1; { int z = 0; } static int i = 0; return i; }
constexpr int destructor_result(int x) { retD(x); static int i = 0; return i; }
constexpr int known_record_call(int x) { retS(1); static int i = 0; return i; }
