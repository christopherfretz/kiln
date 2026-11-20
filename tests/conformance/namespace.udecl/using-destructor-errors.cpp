// [namespace.udecl]/1: a using-declarator cannot name a destructor:
// clang's "using declaration cannot refer to a destructor" at the `~`,
// at class and namespace scope and in a template (WP-1745).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:25 error "using declaration cannot refer to a destructor"
//@ diag: 13:13 error "using declaration cannot refer to a destructor"
//@ diag: 14:43 error "using declaration cannot refer to a destructor"

struct B { ~B(); };
struct D : B { using B::~B; };
namespace N { struct C {}; }
using N::C::~C;
template<class T> struct E : T { using T::~T; };
