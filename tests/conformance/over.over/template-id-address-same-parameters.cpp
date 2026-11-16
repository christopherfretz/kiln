// [over.over]/5 as amended by CWG2918: of the specializations a
// template-id names, one is chosen only when it is more constrained than
// every other, which needs the same parameter-type-lists. `g<int>`
// names `g(int)` and `g(int*)`: neither is chosen, though the second
// template is more specialized (clang agrees). Found running GCC's C++
// testsuite.
//@ expect: compile-fail
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
//@ diag: 21:6 error "variable 'q' with type 'auto' has incompatible initializer of type '<overloaded function type>'"

template <class T> concept C = sizeof(T) >= 1;
template <class T> concept D = C<T> && sizeof(T) <= 64;
struct X {};
template <C T> void f(T);
template <D T> void f(T);
auto p = &f<X>; // the more constrained `f(T)`
template <class T> void g(T);
template <class T> void g(T *);
auto q = &g<int>;
