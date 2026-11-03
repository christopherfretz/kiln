// [class.base.init]/7, [over.over]/1-3, [expr.prim.id.general]/2: a name
// naming an overload set of member functions, some static, used where a
// pointer to function is the target (a base's braced mem-initializer,
// libc++ 22's `__output_buffer<C>{buf, n, __prepare_write, max}`) selects
// the matching static member function. Through an explicit object (`this->g`,
// `o.g`) clang rejects the static winner; no match lists every candidate,
// the last declared first.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 53:22 error "address of overloaded function 'g' does not match required type 'void (int)'"
//@ diag: 40:15 note "candidate function has type mismatch at 1st parameter (expected 'int' but has 'long')"
//@ diag: 39:8 note "candidate function"
//@ diag: 54:10 error "cannot create a non-constant pointer to member function"
//@ diag: 58:39 error "cannot create a non-constant pointer to member function"
struct B {
  using fn = void (*)(B &, unsigned long);
  B(char *, unsigned long, fn) {}
};
struct D : B {
  D() : B{buf, 4, prep} {}
  void prep(unsigned long) {}
  static void prep(B &, unsigned long) {}
  char buf[4];
};
template <class C> struct TB {
  using fn = void (*)(TB &, unsigned long);
  explicit TB(C *, unsigned long, fn, int *) {}
};
template <class C> struct TD : TB<C> {
  TD() : TD{nullptr} {}
  explicit TD(int *m) : TB<C>{small, 4, prep, m} {}
  void prep(unsigned long) {}
  static void prep(TB<C> &, unsigned long) {}
  C small[4];
};
TD<char> td;
void take(void (*)(long));
struct E {
  void g(int);
  static void g(long);
  template <class T> static void t(T);
  void t(int, int);
  void h() {
    void (*p)(long) = g;
    p = (g);
    void (&r)(long) = g;
    take(g);
    void (*q)(char) = t;
    void (*q2)(double) = t<double>;
    (void)p, (void)r, (void)q, (void)q2;
  }
  void bad() {
    void (*p)(int) = g;
    take(this->g);
    (void)p;
  }
};
void outside(E e) { void (*p)(long) = e.g; (void)p; }
