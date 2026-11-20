// [dcl.type.class.deduct]/2, [temp.arg.template]: a template template
// parameter named without template arguments is a placeholder for a
// deduced class type, allowed only where one is (a variable's declared
// type, a new-expression, a functional cast) and diagnosed in the template
// definition elsewhere, as clang. kiln accepted `typeid(T)` and
// `sizeof(T)`; the explicit instantiation then reached code generation.
// Found running GCC's C++ testsuite. Verified against clang 19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:18 error "use of template template parameter 'T' requires template arguments; argument deduction not allowed here"
//@ diag: 26:34 note "template is declared here"
//@ diag: 34:18 error "use of template template parameter 'T' requires template arguments; argument deduction not allowed here"
//@ diag: 26:34 note "template is declared here"
//@ diag: 35:7 error "cannot form pointer to deduced class template specialization type"
namespace std {
class type_info {
public:
  const char *name() const;
};
} // namespace std

template <class T> struct B {
  B(int);
};

template <template <class> class T> struct A {
  void ok() {
    T x(1); // deduced at instantiation
    (void)new T(1);
    (void)T(1);
  }
  void bad() {
    (void)typeid(T).name();
    (void)sizeof(T);
    T *p;
  }
};

template void A<B>::bad();
