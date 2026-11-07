// [dcl.contract.func]/3, [stmt.contract.assert]/1 (P2900R14): an
// attribute-specifier-seq after `pre`, `post` or `contract_assert`
// appertains to the contract assertion. No standard attribute applies to
// one, so kiln ignores them with a warning, as GCC 16.1 does.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ diag: 12:19 warning "attributes are ignored on function contract specifiers"
//@ diag: 13:14 warning "attributes are ignored on function contract specifiers"
//@ diag: 14:33 warning "attributes are ignored on 'contract_assert'"
void q(int a) pre [[deprecated]] (a);
int r() post [[maybe_unused]] (v: v > 0);
void z(int a) { contract_assert [[maybe_unused]] (a > 0); }
