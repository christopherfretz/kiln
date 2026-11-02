// [temp.inst]/14, [except.spec]/13: a destructor whose instantiated
// noexcept-specifier is false is not nothrow-destructible.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:15 error "static assertion failed due to requirement '__is_nothrow_destructible(N<double>)'"
template <class T> struct N { ~N() noexcept(sizeof(T) == 0); };
static_assert(__is_nothrow_destructible(N<double>));
