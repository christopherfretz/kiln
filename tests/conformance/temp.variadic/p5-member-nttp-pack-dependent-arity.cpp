// [temp.param]/17, [temp.arg.general]/1: in a specialization of `P<Ts...>`
// the member template `template<Ts... a> struct A` has one non-type
// parameter per element of `Ts`. A template-id naming it with dependent
// arguments is still checked for arity as clang's
// CheckTemplateArgumentList does: the expanded pack takes at most one
// argument per element, a pack expansion counting as one, and fewer once
// a pack expansion is among them. `P<>` with `Is<A<a...>>` and
// `Q<int>::R<int, int>` with `A<1, u...>` are "too many template
// arguments for class template 'A'" (printed unqualified, as clang
// prints the template, not `P<>::A`).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 24:32 error "too many template arguments for class template 'A'"
//@ diag: 28:5 note "in instantiation of template class 'P<>' requested here"
//@ diag: 22:29 note "template is declared here"
//@ diag: 31:15 error "too many template arguments for class template 'A'"
//@ diag: 30:61 note "template is declared here"
//@ diag: 32:15 error "too few template arguments for class template 'A'"
//@ diag: 30:61 note "template is declared here"
template <class... Ts> struct P {
  template <Ts... a> struct A {};
  template <class> struct Is { static constexpr int v = 0; };
  template <Ts... a> struct Is<A<a...>> { static constexpr int v = 1; };
};
P<int> p1;
P<int, char> p2;
P<> p0;
static_assert(P<int>::Is<P<int>::A<3>>::v == 1);
template <class... Ts> struct O { template <Ts... a> struct A {}; };
O<int, char>::A<1, 2, 3> o1;
O<int, char>::A<1> o2;
