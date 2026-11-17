// [class.conv.fct]/1: a conversion function's decl-specifier-seq contains no
// type specifier and no cv-qualifier: the whole type goes after operator.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:9 error "cannot specify any part of a return type in the declaration of a conversion function; put the complete type after 'operator'"
//@ diag: 12:7 error "conversion function cannot have a return type"
//@ diag: 13:10 error "conversion function must be a non-static member function"
//@ diag: 14:12 error "cannot specify any part of a return type in the declaration of a conversion function; put the complete type after 'operator'"
struct A {
  const operator int();
  int operator long();
  static operator char();
  volatile operator bool();
};
struct B {
  operator short() const;
  operator float() &;
  virtual operator double();
  explicit operator unsigned();
  constexpr operator long long() const { return 0; }
  inline operator signed char();
  operator const int() const;
};
