// GNU _Complex with the half types ([basic.extended.fp]; clang 19
// DeclSpec::Finish): `_Complex _Float16` is a complex type, `_Complex
// __fp16` is invalid and spelled with clang's specifier name `half`.
// Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:1 error "'_Complex half' is invalid"
_Complex _Float16 y;
_Complex __fp16 x;
static_assert(sizeof(y) == 4);
