// [dcl.contract.func]/3, [temp.inst] (P2900R14), spec/contracts.md §7: a
// lambda's contract assertions dump under its call operator; a function
// template's predicates dump dependent in the pattern and substituted in
// each specialization. clang has no node for these, so the golden is
// kiln's.
//@ expect: ast
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ ast-filter: test
namespace test {
auto lam = [](int a) pre(a > 0) post(r: r > 0) { return a; };
template <class T> T tf(const T t) pre(t > T{}) post(r: r >= t) { return t; }
int use() { return tf(2) + lam(1); }
}  // namespace test
