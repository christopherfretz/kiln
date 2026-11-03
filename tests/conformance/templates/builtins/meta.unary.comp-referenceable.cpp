// clang's __is_referenceable trait (the [meta.unary.comp] companion
// predicate libc++ used before clang 22 removed it; spec/sema.md §1.1).
//@ strict-diags
//@ requires: clang<22
static_assert(__is_referenceable(int) && __is_referenceable(void()) && __is_referenceable(int &));
static_assert(!__is_referenceable(void) && !__is_referenceable(void() const));
