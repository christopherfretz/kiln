// [namespace.memdef]/3, [temp.friend]: a friend class template
// declaration that redeclares a hidden friend class template (declared
// only by another friend declaration) does not make it visible to
// ordinary or qualified lookup; one that redeclares a visible template
// leaves it visible.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 24:1 error "no template named 'X'"
//@ diag: 25:1 error "no template named 'Y'"
//@ diag: 26:4 error "no template named 'Z' in namespace 'N'"
struct A { template <class T> friend struct X; };
struct B { template <class T> friend struct X; };
template <class> struct TA { template <class T> friend struct Y; };
template struct TA<int>;
template struct TA<long>;
struct C { template <class T> friend struct Y; };
namespace N {
struct D { template <class T> friend struct Z; };
struct E { template <class T> friend struct Z; };
template <class T> struct V; struct F { template <class T> friend struct V; };
} // namespace N
X<int>* x;
Y<int>* y;
N::Z<int>* z;
N::V<int>* v;
