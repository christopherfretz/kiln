// [dcl.spec.auto.general]/9: inconsistent returns are diagnosed at
// instantiation once the operands' types are known.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:5 error "'auto' in return type deduced as 'int' here but deduced as 'long' in earlier return statement"
//@ diag: 15:29 note "in instantiation of member function 'W<long>::f' requested here"
template <class T> struct W {
  static auto f(bool b) {
    if (b)
      return T();
    return 1;
  }
};
using A = decltype(W<int>::f(true));
using B = decltype(W<long>::f(true));
