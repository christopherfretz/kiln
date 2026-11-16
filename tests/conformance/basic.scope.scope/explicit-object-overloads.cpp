// [basic.scope.scope]/4: explicit and implicit object member functions
// that differ in the object parameter's type or reference, or in the other
// parameters, overload.
//@ std: c++23
struct S {
  void f1() &; void f1(this S&&);
  void f2() const; void f2(this S&);
  void f4(int); void f4(this S&, long);
  void f5(this S&); void f5(this S&&);
  void f6() const &; void f6(this const S&&);
  template<class T> void g(T); template<class T> void g(this S&, T, int);
};
int use(S s) { s.f4(1); s.f5(); return 0; }
