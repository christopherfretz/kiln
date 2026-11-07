// [class.virtual] (clang -Winconsistent-missing-override; WP-682): a
// method whose name is a macro argument is checked where the macro's body
// uses the parameter, so a macro from a system header that declares the
// method (googletest's MOCK_METHOD under -isystem) does not warn, though
// the name was written here. A method written directly still warns.
//@ expect: compile-ok
//@ strict-diags
//@ args: -isystem %{test_dir}/Inputs
//@ diag: 22:8 warning "'d' overrides a member function but is not marked 'override'"
//@ diag: 16:16 note "overridden virtual function is here"
#include <override_macros.h>
struct I {
  virtual void a(int);
  virtual int b(int);
  virtual void c(int);
  virtual void d(int);
};
struct M : I {
  DECLARE_METHOD(void, a);
  DECLARE_METHOD(int, b, override);
  void NAME_OF(c)(int);
  void d(int);
};
