// [temp.explicit]/10, [temp.constr.atomic]/3: a member whose constraint
// check is itself ill-formed (an atomic constraint that is not of type
// bool) is diagnosed once, at the member, and is not instantiated; the
// other members still are (clang `InstantiateClassMembers`:
// `CheckFunctionConstraints` fails, the member is skipped).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:21 error "atomic constraint must be of type 'bool' (found 'const int')"
//@ diag: 13:7 note "while checking constraint satisfaction for function 'f' required here"
struct B { static constexpr int v = 2; };
template <typename E> struct File {
  int f() requires (E::v) { return E::missing; }
  int g() { return 1; }
};
template struct File<B>;
