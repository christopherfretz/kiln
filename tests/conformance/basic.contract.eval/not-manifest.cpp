// [basic.contract.eval]/8, [expr.const] (P2900R14): when an evaluation is
// not manifestly constant-evaluated (here `__builtin_constant_p`'s
// operand), a contract violation under any checking semantic only makes it
// not a constant: no diagnostic. GCC 16.1 instead diagnoses the violation
// (spec/contracts.md §9), so there is no oracle.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ args: -fcontract-evaluation-semantic=observe
//@ xfail(gcc): GCC 16.1 diagnoses a violation in an evaluation that is not manifestly constant-evaluated (spec/contracts.md §9)
constexpr int f(const int a) pre(a > 0) post(r: r != 6) { contract_assert(a != 9); return a + 1; }
static_assert(__builtin_constant_p(f(1)));
static_assert(!__builtin_constant_p(f(-1)));
static_assert(!__builtin_constant_p(f(5)));
static_assert(!__builtin_constant_p(f(9)));
bool b = __builtin_constant_p(f(-1));
