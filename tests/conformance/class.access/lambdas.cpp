// [expr.prim.lambda.closure], [class.access]/2: a lambda's body has the access
// of its enclosing context: a member function's, a friend's; a generic lambda's
// operator() instantiation is checked in that context.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:21 error "'y' is a private member of 'B'"
//@ diag: 13:7 note "implicitly declared private here"
//@ diag: 35:24 error "'y' is a private member of 'B'"
//@ diag: 35:28 note "in instantiation of function template specialization 'outside()::(anonymous class)::operator()<B>' requested here"
//@ diag: 13:7 note "implicitly declared private here"
class B {
  struct P { int v; };
  int y;
  friend void fr();
  friend struct FS;
public:
  void m() {
    auto l = [] { return P{1}.v; };
    (void)l();
    auto g = [](auto x) { return B::P{x}.v; };
    (void)g(1);
    [this] { (void)y; }();
  }
};
void fr() {
  [](B b) { (void)b.y; }(B());
  [](auto b) { (void)b.y; }(B());
  [] { B::P p{1}; (void)p; }();
}
struct FS {
  void n() { [](auto) { B::P p{2}; (void)p; }(1); }
};
void outside() {
  [](B b) { (void)b.y; }(B());
  [](auto b) { (void)b.y; }(B());
}
