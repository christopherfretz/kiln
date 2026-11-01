// [over.match.class.deduct]/4, [over.match.list]/1.1: list-initialization
// tries the initializer-list guides first (the copy-list explicit check
// applies to the one chosen); when none is viable, all guides with the
// elements as arguments.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:3 error "class template argument deduction for 'Y' selected an explicit constructor for copy-list-initialization"
//@ diag: 17:39 note "explicit constructor declared here"
//@ diag: 20:3 error "no viable constructor or deduction guide for deduction of template arguments of 'Z'"
//@ diag: 19:30 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 19:30 note "implicit deduction guide declared as 'template <class T> Z(std::initializer_list<T>) -> Z<T>'"
//@ diag: 19:26 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 19:26 note "implicit deduction guide declared as 'template <class T> Z(Z<T>) -> Z<T>'"
//@ diag: 22:8 error "type 'double' cannot be narrowed to 'int' in initializer list"
//@ diag: 22:8 note "insert an explicit cast to silence this issue"
namespace std { template<class E> class initializer_list { const E *b; decltype(sizeof 0) n; public: constexpr initializer_list() : b(nullptr), n(0) {} }; }
template<class T> struct Y { explicit Y(std::initializer_list<T>); Y(T, T); };
Y y = {1, 2};
template<class T> struct Z { Z(std::initializer_list<T>); };
Z z{1, 2.0};
template<class T> struct W { W(std::initializer_list<T>); W(T, double); };
W w{1, 2.0};
static_assert(__is_same(decltype(w), W<int>));
