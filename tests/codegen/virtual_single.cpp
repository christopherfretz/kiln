// Single inheritance: vtables, class/si RTTI, vptr stores in
// constructors and destructors, virtual calls and the deleting destructor.
// (Every function is noexcept: no EH before WP-100.)
struct Base {
  int x;
  Base() noexcept : x(1) {}
  virtual int get() noexcept { return x; }
  virtual int twice() noexcept { return 2 * get(); }
  virtual ~Base() noexcept {}
};
struct Derived : Base {
  int y;
  Derived() noexcept : y(5) {}
  int get() noexcept override { return x + y; }
};
int call(Base* b) noexcept { return b->get() + b->twice(); }
int test() noexcept {
  Derived d;
  Base b;
  return call(&d) + call(&b);
}
