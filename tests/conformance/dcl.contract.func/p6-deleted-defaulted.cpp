// [dcl.contract.func]/6 (P2900R14): a deleted function and a function
// defaulted on its first declaration cannot have a
// function-contract-specifier-seq. GCC 16.1 accepts both (it diagnoses
// neither); kiln follows the standard text (spec/contracts.md §9).
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ diag: 12:16 error "deleted function cannot have function contract specifiers"
//@ diag: 14:7 error "defaulted function cannot have function contract specifiers"
//@ diag: 15:15 error "deleted function cannot have function contract specifiers"
//@ diag: 16:35 error "defaulted function cannot have function contract specifiers"
void d1(int x) pre(x > 0) = delete;
struct S {
  S() pre(true) = default;
  S(const S&) post(true) = delete;
  bool operator==(const S&) const pre(true) = default;
};
