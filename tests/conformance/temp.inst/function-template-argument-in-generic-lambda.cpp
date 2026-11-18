// [basic.def.odr]/3, /7, [temp.inst]/11: a function template
// specialization named as a non-type template argument in the body of a
// generic lambda inside a function template is odr-used by each
// instantiation of the lambda's call operator, and is instantiated; also
// in a non-generic lambda nested in it, and with a dependent argument.
// The generic lambda is rebuilt as a pattern in each instantiation of the
// enclosing function, and its call operator's instantiations instantiate
// what the original pattern named (WP-1533 (a)). clang 22 and GCC 16.1
// instantiate none of them; the text decides. In
// `decltype(A<&f<int>>{})` the argument is a subexpression of an
// unevaluated operand, not odr-used, and is not instantiated in a
// template (clang 22 agrees; GCC 16.1 instantiates it).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:40 error "type 'char' cannot be used prior to '::' because it has no members"
//@ diag: 27:45 note "in instantiation of function template specialization 'f<char>' requested here"
//@ diag: 31:14 note "in instantiation of function template specialization 'g1<int>' requested here"
//@ diag: 25:40 error "type 'short' cannot be used prior to '::' because it has no members"
//@ diag: 28:50 note "in instantiation of function template specialization 'f<short>' requested here"
//@ diag: 31:25 note "in instantiation of function template specialization 'g2<int>' requested here"
//@ diag: 25:40 error "type 'unsigned int' cannot be used prior to '::' because it has no members"
//@ diag: 29:45 note "in instantiation of function template specialization 'f<unsigned int>' requested here"
//@ diag: 31:36 note "in instantiation of function template specialization 'g3<unsigned int>' requested here"
//@ xfail(clang): clang 22 does not instantiate a specialization named as a template argument in a template body; the text does
template <class T> void f() { typename T::nope x; }
template <void (*)()> struct A {};
template <class T> void g1() { [](auto) { A<&f<char>> a; (void)a; }(0); }
template <class T> void g2() { [](auto) { [] { A<&f<short>> a; (void)a; }(); }(0); }
template <class T> void g3() { [](auto) { A<&f<T>> a; (void)a; }(0); }
template <class T> void g4() { decltype(A<&f<long>>{}) a; (void)a; }
void use() { g1<int>(); g2<int>(); g3<unsigned>(); g4<int>(); }
