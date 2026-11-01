// [over.match.class.deduct]/1.1: an implicit deduction guide takes the
// constructor's parameter types, so a member typedef of the class template
// (libc++ `vector(initializer_list<value_type>, const allocator_type&)`)
// is substituted with the guide's deduced arguments (clang's guide
// transform clones the typedef; the class is not instantiated).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:3 error "no viable constructor or deduction guide for deduction of template arguments of 'C'"
//@ diag: 21:44 note "candidate template ignored: could not match 'V'"
//@ diag: 21:44 note "implicit deduction guide declared as 'template <class T> C(V, int) -> C<T>'"
//@ diag: 21:26 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 21:26 note "implicit deduction guide declared as 'template <class T> C(C<T>) -> C<T>'"
#include <initializer_list>
template<class T> struct A { using value_type = T; A(std::initializer_list<value_type>) {} };
A a{1, 2};
static_assert(__is_same(decltype(a), A<int>));
template<class T> struct Al {};
template<class T, class Al_ = Al<T>> struct B { using value_type = T; using allocator_type = Al_; B(std::initializer_list<value_type>, const allocator_type& = allocator_type()) {} };
B b{1, 2};
static_assert(__is_same(decltype(b), B<int>));
template<class T> struct C { using V = T*; C(V, int) {} };
C c{(long*)0, 1};
static_assert(__is_same(decltype(c), C<long>));
C d{1, 2};
