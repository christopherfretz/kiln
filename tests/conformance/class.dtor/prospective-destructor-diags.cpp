// [class.dtor]/4: overload resolution with an empty argument list selects
// the destructor among the prospective destructors at the end of the class
// definition; the program is ill-formed if it fails (no viable or an
// ambiguous one; clang err_no_viable_destructor / err_ambiguous_destructor).
// A dependent class selects nothing ([class.dtor]/4: "the end of the
// definition of a class" that is not templated), and the more constrained
// candidate wins ([over.match.best.general]/2.6). Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:27 error "destructor of class 'A<int>' is ambiguous"
//@ diag: 34:8 note "in instantiation of template class 'A<int>' requested here"
//@ diag: 31:3 note "candidate function"
//@ diag: 32:3 note "candidate function"
//@ diag: 35:27 error "no viable destructor found for class 'N<int>'"
//@ diag: 39:8 note "in instantiation of template class 'N<int>' requested here"
//@ diag: 36:3 note "candidate function not viable: constraints not satisfied"
//@ diag: 36:17 note "because 'int' does not satisfy 'Big'"
//@ diag: 29:34 note "because 'sizeof(int) > 4' (4 > 4) evaluated to false"
//@ diag: 37:3 note "candidate function not viable: constraints not satisfied"
//@ diag: 37:18 note "because 'sizeof(int) > 8' (4 > 8) evaluated to false"
//@ diag: 41:27 error "no viable destructor found for class 'M<char>'"
//@ diag: 45:9 note "in instantiation of template class 'M<char>' requested here"
//@ diag: 42:3 note "candidate function not viable: constraints not satisfied"
//@ diag: 42:17 note "because 'false' evaluated to false"
// A: both satisfied, neither more constrained (different atomic
// constraints). N, M: none satisfied; M<int> is not instantiated (only a
// pointer). S: Big2 subsumes Big, and either beats the unconstrained one,
// whose `= default` makes S<char> trivially destructible.
template <class T> concept Big = sizeof(T) > 4;
template <class T> struct A {
  ~A() requires true;
  ~A() requires (sizeof(T) > 0);
};
A<int> a;
template <class T> struct N {
  ~N() requires Big<T>;
  ~N() requires (sizeof(T) > 8);
};
N<int> n;
N<int> *pn;
template <class T> struct M {
  ~M() requires false;
};
M<int> *pm;
M<char> mm;
template <class T> concept Big2 = Big<T> && true;
template <class T> struct S {
  ~S() requires Big<T>;
  ~S() requires Big2<T>;
  ~S() = default;
};
static_assert(__is_trivially_destructible(S<char>));
static_assert(!__is_trivially_destructible(S<long>));
