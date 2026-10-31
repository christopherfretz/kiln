// [basic.lookup.qual.general]/1: a name after `::` that is neither
// declared in the global namespace nor a builtin is not found.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:11 error "no member named '__builtin_no_such_thing' in the global namespace"
int a = ::__builtin_no_such_thing(1);
