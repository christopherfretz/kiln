// [class.conv.fct]/9 (CWG1878): a conversion function template shall not
// have a deduced return type, in class or out of line. A non-template
// 'operator auto', a conversion template to 'T', and a member of a class
// template stay valid. WP-1017.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:34 error "'auto' not allowed in declaration of conversion function template"
//@ diag: 15:34 error "'auto' not allowed in declaration of conversion function template"
//@ diag: 16:34 error "'decltype(auto)' not allowed in declaration of conversion function template"
//@ diag: 21:34 error "'auto' not allowed in declaration of conversion function template"
//@ diag: 23:35 error "'auto' not allowed in declaration of conversion function template"
struct S {
  template <typename T> operator auto() const { return 1; }
  template <typename T> operator auto *() const { return nullptr; }
  template <typename T> operator decltype(auto)() const { return 1; }
  operator auto() const { return 1L; }
  template <typename T> operator T() const { return T(); }
};
struct O {
  template <typename T> operator auto() const;
};
template <typename T> O::operator auto() const { return 1; }
template <typename U> struct Q {
  operator auto() const { return U(); }
};
long l = S();
int k = Q<int>();
