// [class.mem.general]/7: the noexcept-specifier of a member function, a
// member function template, and a friend function or friend template
// declared in the class is a complete-class context: it names members
// declared later. [except.spec]/13: needing it before the class is
// complete (outside another complete-class context) is an error. A friend
// redeclaration's specification is compared once known ([except.spec]/5).
// [temp.inst]/14: a function template specialization's dependent
// noexcept-specifier is substituted when needed, in the function's own
// context: a friend may name its class's private members, not those of
// another specialization ([class.friend]/2).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 44:73 error "'x' is a private member of 'TS<long>'"
//@ diag: 54:62 note "in instantiation of exception specification for 'h<long>' requested here"
//@ diag: 50:5 note "declared private here"
//@ diag: 75:26 error "exception specification is not available until end of class definition"
//@ diag: 75:17 error "static assertion failed due to requirement 'noexcept(m())'"
//@ diag: 77:26 error "exception specification is not available until end of class definition"
//@ diag: 77:17 error "static assertion failed due to requirement 'noexcept(fe(*(Early *)nullptr))'"
//@ diag: 83:14 error "exception specification in declaration does not match previous declaration"
//@ diag: 81:5 note "previous declaration is here"
//@ diag: 98:6 error "'gi' is missing exception specification 'noexcept(VT::v)'"
//@ diag: 91:15 note "previous declaration is here"
//@ diag: 105:15 error "'ml' is missing exception specification 'noexcept(Later::x)'"
//@ diag: 104:15 note "previous declaration is here"
int g(int) noexcept;
long g(long);

struct S {
  friend int f(const S& s) noexcept(noexcept(g(s.x))) { return 0; }
  int m() noexcept(noexcept(g(x))) { return 0; }
  template <class U> int mt(U) noexcept(noexcept(g(x))) { return 0; }
  int m2() noexcept(noexcept(m()));
  void body() { static_assert(noexcept(m())); }
private:
  int x;
};
static_assert(noexcept(f(S{})));
static_assert(noexcept(S{}.m()) && noexcept(S{}.mt(1)) && noexcept(S{}.m2()));

template <class T> struct TS {
  friend int f(const TS& s) noexcept(noexcept(g(s.x))) { return 0; }
  template <class U>
  friend int h(const TS& s, const TS<U>& u) noexcept(noexcept(g(s.x + u.x))) { return 0; }
  template <class U> int mt(U u) noexcept(noexcept(g(x + u))) { return 0; }
  int m() noexcept(noexcept(g(x)));
  static int sf(T) noexcept(noexcept(g(sx)));
  template <class U> static int smt(U) noexcept(noexcept(g(sx)));
private:
  T x;
  static T sx;
};
static_assert(noexcept(f(TS<int>{})) && !noexcept(f(TS<long>{})));
static_assert(noexcept(h(TS<int>{}, TS<int>{})) && !noexcept(h(TS<int>{}, TS<long>{})));
static_assert(noexcept(TS<int>{}.mt(1)) && !noexcept(TS<int>{}.mt(1L)));
static_assert(noexcept(TS<int>{}.m()) && !noexcept(TS<long>{}.m()));

// The resolved specification takes part in overload resolution.
int pick(int (*)(int) noexcept);
long pick(int (*)(long));
static_assert(sizeof(pick(&TS<int>::sf)) == sizeof(int));
static_assert(sizeof(pick(&TS<int>::template smt<int>)) == sizeof(int));
int (TS<int>::*pm)() noexcept = &TS<int>::m;

void local() {
  struct L {
    int m() noexcept(noexcept(g(y)));
    int y;
  };
  static_assert(noexcept(L{}.m()));
}

struct Early {
  static int m() noexcept(noexcept(g(sx)));
  static_assert(noexcept(m()));
  friend void fe(Early& e) noexcept(noexcept(e.m())) {}
  static_assert(noexcept(fe(*(Early*)nullptr)));
  static int sx;
};

int fd(const struct R&) noexcept;
struct R {
  friend int fd(const R& r) noexcept(noexcept(g(r.y)));
  long y;
};

// [except.spec]/5 against a friend's delayed specification: a
// redeclaration without one is missing it once it is known; one of a
// class template specialization's friends is instantiated to compare.
template <class T> struct Inst {
  friend void gi(Inst) noexcept(T::v);
  friend void hi(Inst) noexcept(T::v);
};
struct VT { static const bool v = true; };
struct VF { static const bool v = false; };
Inst<VT> ivt;
Inst<VF> ivf;
void gi(Inst<VT>);
void gi(Inst<VF>);
void hi(Inst<VT>) noexcept;
static_assert(noexcept(hi(ivt)));
static_assert(!noexcept(gi(ivf)));
struct Later {
  friend void ml() noexcept(Later::x);
  friend void ml();
  static constexpr bool x = true;
};
