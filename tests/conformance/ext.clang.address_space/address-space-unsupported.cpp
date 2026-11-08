// clang address_space(N) (WP-809): the type qualifier is not implemented,
// so both spellings are WP-805's class-A error rather than silently
// dropped (the address space changes the IR pointer type and the
// mangling). The `[[]]` spelling is named 'clang::address_space' at
// -fclang-compat=22 and 'address_space' at 19.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang implements address_space; kiln rejects it (WP-809)
//@ diag: 12:20 error "'address_space' attribute is not supported by kiln yet"
//@ diag: 13:7 error "address_space' attribute is not supported by kiln yet"

int __attribute__((address_space(1))) *p;
int [[clang::address_space(2)]] *q;
