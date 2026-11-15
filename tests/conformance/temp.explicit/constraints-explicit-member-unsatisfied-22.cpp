// [temp.explicit]/10, clang 22: an explicit instantiation naming a member
// function whose trailing requires-clause is not satisfied has no viable
// candidate (kiln at -fclang-compat=22). clang 19 instantiates the member:
// constraints-explicit-member-unsatisfied.cpp.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ std: c++20
//@ diag: 21:23 error "no viable candidate for explicit instantiation of 'f'"
//@ diag: 17:7 note "candidate function not viable: constraints not satisfied"
//@ diag: 17:20 note "because 'B' does not satisfy 'has_abi'"
//@ diag: 13:55 note "because 'E::abi' would be invalid: no member named 'abi' in 'B'"
template <typename E> concept has_abi = requires { E::abi; };
struct A { static constexpr int abi = 1; };
struct B {};
template <typename E> struct File {
  int f() requires has_abi<E>;
};
template <typename E> int File<E>::f() requires has_abi<E> { return E::abi; }
template int File<A>::f();
template int File<B>::f();
