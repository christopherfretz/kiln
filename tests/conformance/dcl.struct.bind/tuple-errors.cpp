//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:9 error "use of undeclared identifier 'get'"
//@ diag: 28:9 note "in implicit initialization of binding declaration 't'"
//@ diag: 29:8 error "cannot decompose this type; 'std::tuple_size<T2>::value' is not a valid integral constant expression"
//@ diag: 29:8 note "initializer of 'value' is unknown"
//@ diag: 24:69 note "declared here"
//@ diag: 30:9 error "implicit instantiation of undefined template 'std::tuple_element<0, T3>'"
//@ diag: 30:9 note "in implicit initialization of binding declaration 'u'"
//@ diag: 19:44 note "template is declared here"
//@ diag: 31:8 error "type 'T3' decomposes into 2 elements, but only 1 name was provided"
// [dcl.struct.bind]/4 (C++26 /7): for a tuple-like type,
// std::tuple_size<E>::value must be an integral constant expression, a get
// must be found (member template or by argument-dependent lookup), and
// std::tuple_element<i, E>::type must name a type. clang adds the
// "in implicit initialization of binding declaration" context note.
namespace std {
template <class T> struct tuple_size;
template <unsigned long I, class T> struct tuple_element;
}
struct T1 {};
template <> struct std::tuple_size<T1> { static constexpr unsigned long value = 1; };
struct T2 {};
template <> struct std::tuple_size<T2> { static const unsigned long value; };
struct T3 { template <unsigned long I> int get() const { return 0; } };
template <> struct std::tuple_size<T3> { static constexpr unsigned long value = 2; };
void f(T1 t1, T2 t2, T3 t3) {
  auto [t] = t1;
  auto [t2a] = t2;
  auto [u, v] = t3;
  auto [w] = t3;
}
