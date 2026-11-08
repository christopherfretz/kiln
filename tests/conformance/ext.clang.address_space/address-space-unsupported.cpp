// clang address_space(N) (WP-809): the type qualifier is not implemented,
// so both spellings are rejected rather than silently dropped (the
// address space changes the IR pointer type and the mangling).
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang implements address_space; kiln rejects it (WP-809)
//@ diag: 10:20 error "the 'address_space' attribute is not supported yet"
//@ diag: 11:7 error "the 'address_space' attribute is not supported yet"

int __attribute__((address_space(1))) *p;
int [[clang::address_space(2)]] *q;
