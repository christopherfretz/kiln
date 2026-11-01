// [over.match.class.deduct]/1.2: failures among constructor-template
// guides; each note shows the synthesized guide's signature (template
// parameters re-depthed after the class's, inherited default arguments
// as clang's '<null expr>', type-constraints as 'C<>').
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:3 error "no viable constructor or deduction guide for deduction of template arguments of 'B'"
//@ diag: 29:32 note "candidate template ignored: couldn't infer template argument 'N'"
//@ diag: 29:32 note "implicit deduction guide declared as 'template <class T, T N, class U = int> B(T, U = <null expr>) -> B<T>'"
//@ diag: 28:26 note "candidate template ignored: could not match 'B<T>' against 'int'"
//@ diag: 28:26 note "implicit deduction guide declared as 'template <class T> B(B<T>) -> B<T>'"
//@ diag: 36:3 error "no viable constructor or deduction guide for deduction of template arguments of 'K'"
//@ diag: 34:21 note "candidate template ignored: constraints not satisfied [with T = int, U = int]"
//@ diag: 34:12 note "because 'int' does not satisfy 'Small'"
//@ diag: 32:35 note "because 'sizeof(int) == 1' (4 == 1) evaluated to false"
//@ diag: 34:21 note "implicit deduction guide declared as 'template <class T, Small<> U> K(T, U) -> K<T>'"
//@ diag: 33:26 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 33:26 note "implicit deduction guide declared as 'template <class T> K(K<T>) -> K<T>'"
//@ diag: 38:3 error "class template argument deduction for 'E' selected an explicit constructor for copy-list-initialization"
//@ diag: 37:57 note "explicit constructor declared here"
//@ diag: 41:3 error "no viable constructor or deduction guide for deduction of template arguments of 'D'"
//@ diag: 40:92 note "candidate function template not viable: requires 1 argument, but 0 were provided"
//@ diag: 40:92 note "implicit deduction guide declared as 'template <class T, int N = 3> D(T *) -> D<T>'"
//@ diag: 40:26 note "candidate function template not viable: requires 1 argument, but 0 were provided"
//@ diag: 40:26 note "implicit deduction guide declared as 'template <class T> D(D<T>) -> D<T>'"
//@ diag: 40:60 note "candidate function template not viable: requires 3 arguments, but 0 were provided"
//@ diag: 40:60 note "implicit deduction guide declared as 'template <class T, class U = int, C2<> V> D(T, U, V) -> D<T>'"
template<class T> struct B {
  template<T N, class U = int> B(T, U = U());
};
B b(1);
template<class T> concept Small = sizeof(T) == 1;
template<class T> struct K {
  template<Small U> K(T, U);
};
K k(1, 2);
template<class T> struct E { template<class U> explicit E(T, U); };
E e = {1, 2};
template<class T> concept C2 = true;
template<class T> struct D { template<class U = int, C2 V> D(T, U, V); template<int N = 3> D(T*); };
D d;
