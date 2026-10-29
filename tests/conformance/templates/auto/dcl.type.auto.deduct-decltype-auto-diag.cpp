// [dcl.type.auto.deduct]/5, [dcl.spec.auto.general]/2: decltype(auto)
// must be the whole declared type and cannot deduce from a braced list.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:7 error "'decltype(auto)' cannot be combined with other type specifiers"
//@ diag: 10:16 error "cannot form pointer to 'decltype(auto)'"
//@ diag: 11:16 error "cannot form reference to 'decltype(auto)'"
int i;
const decltype(auto) a = 1;
decltype(auto) *b = &i;
decltype(auto) &c = i;
decltype(auto) d{1};
