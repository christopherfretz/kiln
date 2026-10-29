// -Werror=constant-evaluated turns the always-true warning into an error.
//@ args: -Werror=constant-evaluated
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:15 error "'__builtin_is_constant_evaluated' will always evaluate to 'true' in a manifestly constant-evaluated expression"
static_assert(__builtin_is_constant_evaluated());
