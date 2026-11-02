// [over.match.class.deduct]/1.1, [temp.param]/4: an implicit deduction guide
// formed from a constrained constructor template prints each type-constraint
// with its written arguments (`Compat<T>`, `Var<T, int>`, `Compat<T *>`), in
// the guide's own parameter names, as clang does in its candidate notes.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:3 error "no viable constructor or deduction guide for deduction of template arguments of 'D'"
//@ diag: 23:26 note "candidate template ignored: could not match 'D<T>' against 'int'"
//@ diag: 23:26 note "implicit deduction guide declared as 'template <class T> D(D<T>) -> D<T>'"
//@ diag: 24:25 note "candidate function template not viable: requires at least 2 arguments, but 1 was provided"
//@ diag: 24:25 note "implicit deduction guide declared as 'template <class T, Compat<T> R> D(int, R &&, T * = <null expr>) -> D<T>'"
//@ diag: 25:19 note "candidate function template not viable: requires at least 2 arguments, but 1 was provided"
//@ diag: 25:19 note "implicit deduction guide declared as 'template <class T, One<> R> D(char, R &&, T * = <null expr>) -> D<T>'"
//@ diag: 26:27 note "candidate function template not viable: requires at least 2 arguments, but 1 was provided"
//@ diag: 26:27 note "implicit deduction guide declared as 'template <class T, Compat<int> R> D(long, R &&, T * = <null expr>) -> D<T>'"
//@ diag: 27:27 note "candidate function template not viable: requires at least 2 arguments, but 1 was provided"
//@ diag: 27:27 note "implicit deduction guide declared as 'template <class T, Var<T, int> R> D(short, R &&, T * = <null expr>) -> D<T>'"
//@ diag: 28:36 note "candidate function template not viable: requires at least 2 arguments, but 1 was provided"
//@ diag: 28:36 note "implicit deduction guide declared as 'template <class T, Compat<T *> R, One<> ...Q> D(double, R &&, T * = <null expr>) -> D<T>'"
template<class R, class T> concept Compat = sizeof(R) == sizeof(T);
template<class R> concept One = true;
template<class R, class... Ts> concept Var = true;
template<class T> struct D {
  template<Compat<T> R> D(int, R&&, T* = nullptr);
  template<One R> D(char, R&&, T* = nullptr);
  template<Compat<int> R> D(long, R&&, T* = nullptr);
  template<Var<T, int> R> D(short, R&&, T* = nullptr);
  template<Compat<T*> R, One... Q> D(double, R&&, T* = nullptr);
};
D d(1);
