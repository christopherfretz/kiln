// [namespace.udecl]/3: a using-declaration shall not name a namespace
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:10 error "using declaration cannot refer to a namespace"
//@ diag: 9:7 note "did you mean 'using namespace'?"
//@ diag: 11:9 error "using declaration cannot refer to a namespace"
//@ diag: 11:7 note "did you mean 'using namespace'?"
namespace N { namespace M { int z; } }
using N::M;
namespace A = N;
using ::A;
