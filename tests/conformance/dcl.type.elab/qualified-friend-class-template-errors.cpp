// [dcl.type.elab]/4, [temp.friend]/1: a qualified friend class template
// declaration names an existing class template of the nominated scope;
// without the friend declaration, access is denied.
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:45 error "redefinition of 'Q' as different kind of symbol"
//@ diag: 14:22 note "previous definition is here"
//@ diag: 16:45 error "friend declaration of 'Missing' does not match any declaration in namespace 'N'"
//@ diag: 18:56 error "friend declaration of 'Other' does not match any declaration in namespace 'N'"
//@ diag: 20:65 error "'x' is a private member of 'K'"
//@ diag: 18:15 note "implicitly declared private here"
namespace N { template <class T> struct Tr; }
namespace M { struct Q; }
class F { template <class> friend struct M::Q; };
class G { template <class> friend struct N::Missing; };

class K { int x = 1; template <class> friend struct N::Other; };
namespace N { template <class T> struct Other; }
template <class T> struct N::Tr { static int g(K &k) { return k.x; } };
int u(K &k) { return N::Tr<int>::g(k); }
