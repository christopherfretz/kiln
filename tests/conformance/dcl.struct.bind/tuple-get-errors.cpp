//@ expect: compile-fail
//@ strict-diags
//@ diag: 38:9 error "no matching function for call to 'get'"
//@ diag: 38:9 note "in implicit initialization of binding declaration 'a'"
//@ diag: 33:33 note "candidate function template not viable: no known conversion from 'T1' to 'T3 &' for 1st argument"
//@ diag: 38:9 error "implicit instantiation of undefined template 'std::tuple_element<0, T1>'"
//@ diag: 27:44 note "template is declared here"
//@ diag: 39:9 error "no matching function for call to 'get'"
//@ diag: 39:9 note "in implicit initialization of binding declaration 'c'"
//@ diag: 33:33 note "candidate function template not viable: expects an lvalue for 1st argument"
//@ diag: 39:9 error "implicit instantiation of undefined template 'std::tuple_element<0, T3>'"
//@ diag: 27:44 note "template is declared here"
//@ diag: 40:10 error "implicit instantiation of undefined template 'std::tuple_element<0, T3>'"
//@ diag: 40:10 note "in implicit initialization of binding declaration 'e'"
//@ diag: 27:44 note "template is declared here"
//@ diag: 41:10 error "reference to type 'std::tuple_element<0UL, T4>::type' (aka 'int') could not bind to an rvalue of type 'void'"
//@ diag: 41:10 note "in implicit initialization of binding declaration 'h'"
//@ requires: clang>=22
// clang 22 prints the sema-built index as 0UL
// [dcl.struct.bind]/4 (C++26 /7): errors in the implicit get<i> calls and
// in std::tuple_element<i, E>::type. As clang does, a failed get call is
// recovered and the next binding is still checked, while a missing
// tuple_element stops the declaration; a get returning void cannot
// initialize the binding's reference.
namespace std {
template <class T> struct tuple_size;
template <unsigned long I, class T> struct tuple_element;
}
struct T1 {};
template <> struct std::tuple_size<T1> { static constexpr unsigned long value = 2; };
struct T3 {};
template <> struct std::tuple_size<T3> { static constexpr unsigned long value = 2; };
template <unsigned long I> void get(T3 &);
struct T4 { template <unsigned long I> void get(); };
template <> struct std::tuple_size<T4> { static constexpr unsigned long value = 2; };
template <unsigned long I> struct std::tuple_element<I, T4> { using type = int; };
void f(T1 t1, T3 t3, T4 t4) {
  auto [a, b] = t1;
  auto [c, d] = t3;
  auto &[e, g] = t3;
  auto &[h, i] = t4;
}
