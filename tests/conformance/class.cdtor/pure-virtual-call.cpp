// [class.cdtor]/4, [class.abstract]/6: a virtual call of a pure virtual function
// on 'this' from a constructor (including its mem-initializers) or destructor
// has undefined behavior: clang's -Wcall-to-pure-virtual-from-ctor-dtor, also
// from a derived class's constructor. A qualified name, a member function that
// is not a constructor or destructor, and an object other than 'this' itself
// ('(*this).f()') do not warn.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 23:9 warning "call to pure virtual member function 'f' has undefined behavior; overrides of 'f' in subclasses are not available in the constructor of 'A'"
//@ diag: 20:3 note "'f' declared here"
//@ diag: 24:10 warning "call to pure virtual member function 'f' has undefined behavior; overrides of 'f' in subclasses are not available in the destructor of 'A'"
//@ diag: 20:3 note "'f' declared here"
//@ diag: 24:27 warning "call to pure virtual member function 'g' has undefined behavior; overrides of 'g' in subclasses are not available in the destructor of 'A'"
//@ diag: 21:3 note "'g' declared here"
//@ diag: 25:14 warning "call to pure virtual member function 'g' has undefined behavior; overrides of 'g' in subclasses are not available in the constructor of 'A'"
//@ diag: 21:3 note "'g' declared here"
//@ diag: 30:9 warning "call to pure virtual member function 'f' has undefined behavior; overrides of 'f' in subclasses are not available in the constructor of 'A'"
//@ diag: 20:3 note "'f' declared here"
struct A {
  virtual void f() = 0;
  virtual int g() const = 0;
  void h();
  A() { f(); }
  ~A() { this->f(); (void)g(); A::f(); }
  A(int) : x(g()) { (*this).f(); h(); }
  A(char) { struct L { void m() {} }; [[maybe_unused]] int y = 0; }
  int x = 0;
};
struct B : A {
  B() { f(); }
};
void A::h() { f(); }
