// [meta.trans.sign]/1: make_signed and make_unsigned need a non-bool
// integral or enumeration type (spec/sema-templates.md §9.2).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:9 error "'make_signed' is only compatible with non-bool integers and enum types, but was given 'bool'"
//@ diag: 11:9 error "'make_unsigned' is only compatible with non-bool integers and enum types, but was given 'float'"
//@ diag: 12:9 error "'make_signed' is only compatible with non-bool integers and enum types, but was given 'S'"
//@ diag: 13:9 error "'make_unsigned' is only compatible with non-bool integers and enum types, but was given 'int *'"
struct S {};
typedef __make_signed(bool) A;
typedef __make_unsigned(float) Bf;
typedef __make_signed(S) C;
typedef __make_unsigned(int *) D;
typedef __make_signed(long) ok;
