// [temp.deduct.general]/5 (CWG2369): the associated constraints are
// checked once the template arguments are known, before the
// non-dependent parameters' conversions that may use a user-defined
// conversion and before the function type is substituted, so an
// unsatisfied constraint stops a conversion through a conversion
// function template whose default argument would be a hard error.
// clang 22 first converts the arguments that need no user-defined
// conversion; when one of those fails, it reports the first bad
// conversion of all. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 37:11 error "no matching function for call to 'h'"
//@ diag: 33:43 note "candidate template ignored: constraints not satisfied [with T = int]"
//@ diag: 33:29 note "because 'small<int>' evaluated to false"
//@ diag: 38:11 error "no matching function for call to 'k'"
//@ diag: 34:43 note "candidate function template not viable: no known conversion from 'int' to 'void *' for 2nd argument"
//@ diag: 39:11 error "no matching function for call to 'm'"
//@ diag: 35:43 note "candidate function template not viable: no known conversion from 'std::nullptr_t' to 'S' for 2nd argument"
template <class T> struct Z { typedef typename T::x xx; };
template <class T> concept HasA = requires { typename T::A; };
struct A {}; // `A::A` names the injected-class-name
template <HasA T> typename Z<T>::xx f(void *, T);
template <class T> void f(int, T);
struct Conv {
  template <class T, class = typename Z<T>::xx> operator T *();
  operator int();
};
void ok() { f(1, A()); f(Conv(), 42); }

template <class T> constexpr bool small = sizeof(T) < 2;
struct S { S(int); };
template <class T> requires small<T> void h(T, S);
template <class T> requires small<T> void k(T, void *);
template <class T> requires small<T> void m(T, S, void *);
int e0 = (h<char>('a', 2), 0);
int e1 = (h(1, 2), 0);
int e2 = (k(1, 2), 0);
int e3 = (m(1, nullptr, 3), 0);
