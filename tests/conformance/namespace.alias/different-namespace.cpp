// [namespace.alias]/2: a namespace alias may be redeclared only to denote
// the same namespace
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:11 error "redefinition of 'A' as an alias for a different namespace"
//@ diag: 10:11 note "previously defined as an alias for 'N'"
//@ diag: 13:11 error "redefinition of 'C' as an alias for a different namespace"
//@ diag: 12:11 note "previously defined as an alias for 'M'"
namespace N { namespace M {} }
namespace A = N;
namespace A = N::M;
namespace C = N::M;
namespace C = N;
namespace D = A;
namespace D = N;
