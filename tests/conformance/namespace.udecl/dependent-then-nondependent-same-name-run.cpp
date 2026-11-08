// [namespace.udecl]/11, [temp.res.general]/1: a using-declarator that names a
// member of a dependent base (`using A::Check;`) names nothing until the
// template is instantiated, so a later non-dependent `using P::Check;` in the
// same class does not conflict with it; in the instantiation the two
// introduce overloads. CMake's export command declares its argument structs
// this way, in a local class of a function template (WP-811).
//@ expect: run
//@ stdout: 1 2 3
extern "C" int printf(const char*, ...);
struct Status {};
struct ParseResult { int Check(const char*, int* = nullptr) const { return 2; } };
struct Info { virtual int Check(Status&) const { return 0; } };
struct PkgInfo : Info { int Check(Status&) const override { return 1; } };

template <typename A> struct Args : A, ParseResult {
  using A::Check;
  using ParseResult::Check;
};

template <typename A> static int local() {
  struct L : public A, public ParseResult {
    using A::Check;
    using ParseResult::Check;
  };
  L l; Status s;
  return l.Check(s) + l.Check("x");
}

int main() {
  Args<PkgInfo> a; Status s;
  printf("%d %d %d\n", a.Check(s), a.Check("x"), local<PkgInfo>());
}
