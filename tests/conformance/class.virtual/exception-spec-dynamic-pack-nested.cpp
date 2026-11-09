// C++14 [except.spec]/5: the dynamic exception specification of an overridden
// function is compared after its pack expands (the base is a specialization), and
// in nested classes once the outermost class is complete (Y is incomplete when X
// names it). An implicit destructor allows what its bases' destructors allow.
// Found by the clang negative-test run (WP-912), fixed by WP-1017.
//@ std: c++11
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 14:16 note "overridden virtual function is here"
//@ diag: 30:10 error "exception specification of overriding function is more lax than base version"
//@ diag: 26:18 note "overridden virtual function is here"
template <class... Ts> struct Pack {
  virtual void f() throw(Ts...);
};
struct PackOk : Pack<int, float> {
  void f() throw(int);
};
struct PackBad : Pack<int, float> {
  void f() throw(int, float, double);
};

struct Outer {
  struct X {
    virtual void f() throw(Y);
    virtual void g() throw(Y);
  };
  struct Y : X {
    void f() throw(Y);
    void g() throw(int);
  };
};
struct Implicit {
  struct X {
    virtual ~X() throw(Y);
  };
  struct Y : X {};
};
