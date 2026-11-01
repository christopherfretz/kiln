// [dcl.fct]/6, [class.this]/1: an explicit object parameter is the first
// parameter of a non-static, non-virtual member function; it has no
// default argument and is not a pack, and `this` is not usable in the body
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:8 error "an explicit object parameter cannot appear in a non-member function"
//@ diag: 18:17 error "an explicit object parameter cannot appear in a static function"
//@ diag: 19:18 error "an explicit object parameter cannot appear in a virtual function"
//@ diag: 20:23 error "an explicit object parameter can only appear as the first parameter of the function"
//@ diag: 21:19 error "the explicit object parameter cannot have a default argument"
//@ diag: 22:32 error "the explicit object parameter cannot be a function parameter pack"
//@ diag: 26:28 error "invalid use of 'this' in a function with an explicit object parameter"
//@ diag: 27:30 error "invalid use of 'this' in a function with an explicit object parameter"


void f(this int);
struct S {
  static void g(this S);
  virtual void h(this S&);
  void i(int, this S& s);
  void l(this S&& = S());
  template <class... T> void m(this T...);
  void n(this S&);
  void o(this S&);
};
void S::n(this S&) { (void)this; }
void S::o(this S& s) { (void)this; }
