// [basic.def.odr]/3, /7, [temp.inst]/11: a template argument is a
// potentially-evaluated expression, so a function template specialization
// named as a non-type template argument is odr-used and instantiated,
// also when the argument is written in a function template's body and
// converted once, at its definition: each instantiation of that body
// instantiates it (a pointer, a reference and a pointer to member
// parameter). clang 22 instantiates it only outside a template; the text
// and GCC 16 decide. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ xfail(clang): clang 22 does not instantiate a specialization named as a template argument in a template body; the text and GCC 16 do
//@ diag: 21:32 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 29:9 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 34:14 note "in instantiation of function template specialization 'g<0>' requested here"
//@ diag: 21:32 error "type 'char' cannot be used prior to '::' because it has no members"
//@ diag: 30:10 note "in instantiation of function template specialization 'f<char>' requested here"
//@ diag: 34:14 note "in instantiation of function template specialization 'g<0>' requested here"
//@ diag: 23:34 error "type 'long' cannot be used prior to '::' because it has no members"
//@ diag: 31:12 note "in instantiation of function template specialization 'S::m<long>' requested here"
//@ diag: 34:14 note "in instantiation of function template specialization 'g<0>' requested here"
template <class T> void f(T) { T::fail; }
struct S {
  template <class T> void m(T) { T::fail; }
};
template <void (*P)(int)> struct ByPtr {};
template <void (&R)(char)> void by_ref() {}
template <void (S::*M)(long)> struct ByMember {};
template <int> void g() {
  ByPtr<f> a;
  by_ref<f>();
  ByMember<&S::m> b;
  (void)a; (void)b;
}
void use() { g<0>(); }
