// [class.friend]/4, [temp.friend]/1.2: a constructor, destructor or
// conversion function of another class can be a friend; it is matched by
// type as an out-of-line member declaration is, a constructor template
// specialization deduced from the type ([temp.deduct.decl]), and the
// friendship grants it access to private members (GCC 16.1, clang 22).
// Found running GCC's C++ testsuite (WP-1466).
//@ expect: run
//@ std: c++17
//@ strict-diags
struct B;
struct A {
  A(int);
  A(const B &);
  template<class T> A(T *);
  ~A();
  operator int() const;
  int v;
};
struct B {
  friend A::A(int);
  friend A::A(const B &) noexcept(false);
  friend A::A(B *);
  friend A::~A();
  friend A::operator int() const;
private:
  int secret = 4;
};
int dtor_seen;
A::A(int) { B b; v = b.secret; }
A::A(const B &b) : v(b.secret + 1) {}
template<class T> A::A(T *p) : v(p->secret + 2) {}
A::~A() { B b; dtor_seen += b.secret; }
A::operator int() const { B b; return v + b.secret; }
int main() {
  B b;
  int r;
  {
    A a1(1), a2(b), a3(&b);
    r = int(a1) + int(a2) + int(a3);
  }
  return r == 8 + 9 + 10 && dtor_seen == 12 ? 0 : 1;
}
