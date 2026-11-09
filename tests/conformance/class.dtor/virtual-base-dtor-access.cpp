// [class.dtor]/13 with [class.access]: a destructor calls the destructors
// of the virtual bases of its class, inherited ones included, which must be
// accessible from it. A befriended class and a public destructor are fine.
// Fixed by WP-1017.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:8 error "inherited virtual base class 'P' has private destructor"
//@ diag: 13:3 note "implicitly declared private here"
//@ diag: 28:8 error "inherited virtual base class 'Q' has private destructor"
//@ diag: 22:13 note "declared private here"
class P {
  ~P();
};
class Q {
protected:
  ~Q();
};
struct V1 : virtual P {
  ~V1();
};
struct V2 : private virtual Q {
  ~V2();
};
struct D1 : V1 {
  ~D1() {}
};
struct D2 : V2 {
  ~D2() {}
};

class F {
  ~F();
  friend struct G;
};
struct W : virtual F {
  ~W();
};
struct G : W {
  ~G() {}
};
struct U {
  virtual ~U();
};
struct X1 : virtual U {};
struct X2 : X1 {
  ~X2() {}
};
