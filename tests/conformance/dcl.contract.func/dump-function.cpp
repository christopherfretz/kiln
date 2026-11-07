// [dcl.contract.func] (P2900R14), spec/ast-dump.md, spec/contracts.md §7:
// a function's contract assertions dump as `ContractAssertStmt` children
// of the FunctionDecl after its parameters and before its body, with
// their kind (pre, post); a postcondition's result name is a
// `ContractResultNameDecl` child naming a const lvalue of the return type,
// and a parameter odr-used in a predicate is a const lvalue there. The
// first declaration carries the sequence; a redeclaration repeats it.
// clang has no node for these, so the golden is kiln's.
//@ expect: ast
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ ast-filter: test
namespace test {
int f(const int x) pre(x > 0) post(r: r > x);
int f(const int x) pre(x > 0) post(r: r > x) { return x + 1; }
int g(int x) pre(x != 0);
int g(int x) { return x; }
struct S {
  int m;
  int get(int a) const pre(a > m) { return a; }
};
}  // namespace test
