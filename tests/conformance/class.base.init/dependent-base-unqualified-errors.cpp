// [class.base.init]/2, [temp.dep.general]/3: an unqualified mem-initializer-id
// of a constructor in a template is looked up when the definition is parsed;
// a dependent base is not searched, so `B(1)` for base `B<T>` (and a name
// that is neither a member nor a type) is ill-formed at the definition. The
// constructor's instantiations report nothing more.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:46 error "member initializer 'Foo' does not name a non-static data member or base class"
//@ diag: 16:46 error "member initializer 'B' does not name a non-static data member or base class"
//@ diag: 19:34 error "member initializer 'B' does not name a non-static data member or base class"
//@ diag: 21:46 error "member initializer 'Fn' does not name a non-static data member or base class"
//@ diag: 22:66 error "member initializer 'B' does not name a non-static data member or base class"

template <class T> struct B { B(int) {} };
template <class T> struct D1 : B<T> { D1() : Foo(1) {} };
template <class T> struct D2 : B<T> { D2() : B(1) {} };
D2<int> d2;
template <class T> struct D3 : B<T> { D3(); };
template <class T> D3<T>::D3() : B(1) {}
int Fn(int);
template <class T> struct D4 : B<T> { D4() : Fn(1) {} };
template <class T> struct D5 : B<T> { template <class U> D5(U) : B(1) {} };
D3<int> d3;
D5<int> d5(0);

// Found at the definition: a member, a member typedef, the base named
// with its arguments, a template parameter.
template <class T> struct E1 : B<T> { int B; E1() : B(1) {} };
template <class T> struct E2 : B<T> { typedef ::B<T> Base; E2() : Base(1) {} };
template <class T> struct E3 : B<T> { E3() : ::B<T>(1) {} };
template <class T> struct E4 : T { E4() : T(1) {} };
E2<int> e2;
E3<int> e3;
E4<B<int>> e4;
