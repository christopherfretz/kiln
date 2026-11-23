// The declaration-extent table (spec/ast.md §19.2, §19.3): a
// postcondition's result name ([dcl.contract.res]) is a declaration with
// a row: its result-name-introducer, from the identifier to the `:`;
// each postcondition's is its own (WP-2630).
//@ decl-extents: contract-result.extents
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ std: c++26
int f() post (r: r > 0);
int g(const int a) post (res: res > a) post (r2: true);
struct M {
  int h() post (r: r != 0) { return 1; }
};
