// [namespace.alias]/2, [basic.scope.declarative]: an alias name cannot be
// another entity's name
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:11 error "redefinition of 'N'"
//@ diag: 9:11 note "previous definition is here"
//@ diag: 13:11 error "redefinition of 'v' as different kind of symbol"
//@ diag: 12:5 note "previous definition is here"
namespace N {}
namespace O {}
namespace N = O;
int v;
namespace v = N;
