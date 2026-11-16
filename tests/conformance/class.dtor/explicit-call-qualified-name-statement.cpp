// [class.dtor]/14, CWG272: `X::~X();` in a member function of `X` (or of a
// class derived from `Y`, `Y::~Y();`) is an explicit destructor call
// through the implicit object, an expression statement; a destructor is
// never declared at block scope (GCC 16.1, clang 22). Found running GCC's
// C++ testsuite (WP-1466).
//@ expect: compile-ok
//@ std: c++17
//@ strict-diags
struct Y { ~Y() {} };
struct X : Y {
  ~X() {}
  void f() {
    X::~X();
    Y::~Y();
    this->X::~X();
  }
};
namespace N { struct Z { ~Z() {} void g() { N::Z::~Z(); ::N::Z::~Z(); } }; }
