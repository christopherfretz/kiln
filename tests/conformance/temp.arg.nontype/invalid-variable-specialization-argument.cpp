// [temp.arg.nontype]/1, [temp.inst]/2: a non-type template argument
// naming a variable whose initializer names a variable template
// specialization that failed to instantiate (here a local class member of
// an invalid union type) is not diagnosed again; clang's reference to the
// invalid declaration has errors (libc++ `expected<void, int&>`'s
// `__allow_reusing_expected_tail_padding`). A variable whose initializer
// is merely not constant is still diagnosed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:32 error "union member 'e' has reference type 'int &'"
//@ diag: 19:60 note "in instantiation of template class 'U<int &>' requested here"
//@ diag: 19:54 note "in instantiation of member class 'X' requested here"
//@ diag: 19:45 note "while substituting into a lambda expression here"
//@ diag: 21:22 note "in instantiation of variable template specialization 'fits<U<int &>>' requested here"
//@ diag: 29:3 error "non-type template argument is not a constant expression"
//@ diag: 29:3 note "initializer of 'cnc' is not a constant expression"
//@ diag: 28:12 note "declared here"
template <class E> union U { E e; };
template <class A> constexpr bool fits = [] { struct X { A a; }; return true; }();
template <bool> struct C {};
constexpr bool put = fits<U<int &>>;
constexpr bool reuse = !put;
C<put> c1;
C<reuse> c2;

bool g();
bool nc = g();
const bool cnc = nc;
C<cnc> c3;
