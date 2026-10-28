// [namespace.udir]/6: names from two nominated namespaces are ambiguous
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:9 error "reference to 'v' is ambiguous"
//@ diag: 7:19 note "candidate found by name lookup is 'A::v'"
//@ diag: 8:19 note "candidate found by name lookup is 'B::v'"
namespace A { int v; }
namespace B { int v; }
using namespace A;
using namespace B;
int w = v;
