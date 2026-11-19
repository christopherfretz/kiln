// [expr.prim.req.nested]/1, [temp.constr.atomic]/3, [temp.arg.general]/9:
// a nested requirement naming a local parameter of the requires-expression
// whose constraint-expression does not substitute as a whole (a concept's
// default argument is invalid for the argument) is checked atom by atom,
// with the local parameter substituted: no atom names the failed
// parameter, so the requirement and the concept are satisfied. clang 22
// and GCC 16.1 agree; clang 19 rejects.
//@ expect: compile-ok
//@ std: c++20
//@ requires: clang>=22
template <class T, class U = typename T::type> concept unary = sizeof(T) > 0;
template <class T>
concept wrapped = requires(T t) { requires unary<decltype(t)>; };
static_assert(wrapped<int>);

template <class T>
concept wrapped_ref = requires(T& t) {
  requires unary<decltype(t)>;
  requires sizeof(t) == sizeof(T);
};
static_assert(wrapped_ref<long>);
