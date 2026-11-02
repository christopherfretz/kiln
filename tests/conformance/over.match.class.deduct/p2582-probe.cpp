// [over.match.class.deduct]/1.3 (C++23, P2582R1): class template argument
// deduction uses the constructors a derived class template inherits.
// WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ xfail(clang): neither clang 19 nor clang 22 implements P2582
template <class T> struct B { B(T) {} };
template <class T> struct D : B<T> { using B<T>::B; };
D d(42);
static_assert(__is_same(decltype(d), D<int>));
