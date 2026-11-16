// [temp.deduct.type]/20: before C++17 a template parameter's type is
// deduced from a value only through an array bound (`std::size_t`); the
// value of a template-id's argument (`A<0>` for `A<N>`) leaves `T` of
// `template<class T, T N>` undeduced, so the deleted template is no
// candidate and `foo(void*)` is called. From C++17 `T` is deduced as
// `int` and the call selects the deleted function. Found running GCC's
// C++ testsuite.
//@ expect: compile-ok
//@ std: c++11

template <int N> struct A;
template <class T, T N> int foo(A<N> *) = delete;
void foo(void *);
void bar(A<0> *p) { foo(p); }
