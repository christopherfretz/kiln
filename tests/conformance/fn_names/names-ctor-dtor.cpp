// Function-name spans (spec/ast.md §24.4): a constructor's and a
// destructor's name is the class name, `class-name`, the `~` outside the
// entry.
//@ fn-names: names-ctor-dtor.names
//@ std: c++20
namespace ns {
struct S {
  S() { (void)__PRETTY_FUNCTION__; (void)__FUNCTION__; }
  ~S() { (void)__PRETTY_FUNCTION__; (void)__func__; }
};
template <class T> struct C {
  C() { (void)__PRETTY_FUNCTION__; }
  template <class U> C(U *) { (void)__PRETTY_FUNCTION__; }
  virtual ~C() { (void)__PRETTY_FUNCTION__; }
};
}
void use() { ns::S s; ns::C<int> c; int i; ns::C<int> d(&i); }
