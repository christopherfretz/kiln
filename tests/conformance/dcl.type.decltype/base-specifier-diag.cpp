// [class.derived.general]/2: the type named by a decltype-specifier base
// must be a class type; `decltype(auto)` is not a class-or-decltype.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:12 error "base specifier must name a class"
//@ diag: 9:12 error "'decltype(auto)' not allowed here"
int i;
struct X : decltype(i) {};
struct Y : decltype(auto) {};
