// [class.access.general]: a member redeclared within its class definition
// keeps the access of its first declaration; an opaque-enum-declaration
// with another access is ill-formed. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++20
//@ diag: 15:8 error "'F' redeclared with 'private' access"
//@ diag: 13:8 note "previously declared 'public' here"
//@ diag: 18:14 error "'G' redeclared with 'public' access"
//@ diag: 16:14 note "previously declared 'private' here"
class C {
public:
  enum F : int;
private:
  enum F : int { a };
  enum class G;
public:
  enum class G { b };
  // Valid: the same access.
  enum H : int;
  enum H : int { c };
};
class D {
  enum class E;
  enum class E { y };
};
