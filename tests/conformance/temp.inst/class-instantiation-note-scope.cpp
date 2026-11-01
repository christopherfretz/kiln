// [temp.inst]: the "in instantiation of template class" note names the
// class as clang prints its type: scoped, without inline namespaces.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:44 error "static assertion failed due to requirement 'sizeof(int) == 0': S"
//@ diag: 27:12 note "in instantiation of template class 'ns::S<int>' requested here"
//@ diag: 24:54 note "expression evaluates to '4 == 0'"
//@ diag: 25:55 error "static assertion failed due to requirement 'sizeof(int) == 0': I"
//@ diag: 28:15 note "in instantiation of template class 'ns::O::I<int>' requested here"
//@ diag: 25:65 note "expression evaluates to '4 == 0'"
//@ diag: 29:44 error "static assertion failed due to requirement 'sizeof(unsigned long) == 0': G"
//@ diag: 31:7 note "in instantiation of template class 'G<unsigned long>' requested here"
//@ diag: 29:54 note "expression evaluates to '8 == 0'"
//@ diag: 32:76 error "static assertion failed due to requirement 'sizeof(char) == 0': In"
//@ diag: 33:20 note "in instantiation of template class 'Out<int>::In<char>' requested here"
//@ diag: 32:86 note "expression evaluates to '1 == 0'"
//@ diag: 34:56 error "static assertion failed due to requirement 'sizeof(int) == 0': A"
//@ diag: 35:8 note "in instantiation of template class '(anonymous namespace)::A<int>' requested here"
//@ diag: 34:66 note "expression evaluates to '4 == 0'"
//@ diag: 36:85 error "static assertion failed due to requirement 'sizeof(int) == 0': vec"
//@ diag: 37:15 note "in instantiation of template class 'std::vec<int>' requested here"
//@ diag: 36:95 note "expression evaluates to '4 == 0'"
namespace ns {
template<class T> struct S { static_assert(sizeof(T) == 0, "S"); };
struct O { template<class T> struct I { static_assert(sizeof(T) == 0, "I"); }; };
}
ns::S<int> s;
ns::O::I<int> i;
template<class T> struct G { static_assert(sizeof(T) == 0, "G"); };
typedef unsigned long sz;
G<sz> g;
template<class T> struct Out { template<class U> struct In { static_assert(sizeof(U) == 0, "In"); }; };
Out<int>::In<char> oi;
namespace { template<class T> struct A { static_assert(sizeof(T) == 0, "A"); }; }
A<int> an;
namespace std { inline namespace __1 { template<class T> struct vec { static_assert(sizeof(T) == 0, "vec"); }; } }
std::vec<int> v;
