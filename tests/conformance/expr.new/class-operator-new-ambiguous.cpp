// [expr.new]/12, [class.member.lookup]/6: the class-scope allocation
// function found in two base classes is ambiguous. Found running GCC's C++
// testsuite.
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:18 error "member 'operator new' found in multiple base classes of different types"
//@ diag: 10:18 note "member found by ambiguous name lookup"
//@ diag: 11:18 note "member found by ambiguous name lookup"
struct P { void *operator new(decltype(sizeof 0)); };
struct Q { void *operator new(decltype(sizeof 0)); };
struct R : P, Q {};
R *mk() { return new R; }
