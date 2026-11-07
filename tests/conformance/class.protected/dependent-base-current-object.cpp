// [class.protected]/1, [temp.dep.type]/6, [namespace.udecl]/19: in a class
// template whose base is dependent, a protected member of a non-dependent
// indirect base named through `*this` (a member using-declaration
// `using B::m;`, an implicit `B::m`) is re-checked in the instantiation
// with the instantiated class as the object class, so it is accessible;
// an unrelated object (`b.m`) is still rejected there. Reduced from
// Catch2 v3.5.0's clara BasicResult (`using ResultBase::m_type;`).
// Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:26 error "'m' is a protected member of 'B'"
//@ diag: 33:34 note "in instantiation of member function 'R<int>::g' requested here"
//@ diag: 18:7 note "can only access this member on an object of type 'R<int>'"
class B {
protected:
  B(int v) : m(v) {}
  void h() {}
  int m;
};
template <class T> struct M : B { M() : B(1) {} };

template <class T> struct R : M<T> {
  using B::m;
  using B::h;
  int f() { h(); return B::m; }
  int k() { return this->B::m; }
  int g(B &b) { return b.m; }
};

int use() {
  R<int> r;
  B &b = r;
  return r.f() + r.k() + r.m + r.g(b);
}
