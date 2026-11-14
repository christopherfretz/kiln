// [temp.explicit]/10 skips an unsatisfied member only for a class's explicit
// instantiation; naming the member itself instantiates its body (clang 19;
// clang 22 rejects the unsatisfied candidate, which kiln does not follow yet).
//@ requires: clang<22
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:72 error "no member named 'abi' in 'B'"
//@ diag: 18:23 note "in instantiation of member function 'File<B>::f' requested here"
template <typename E> concept has_abi = requires { E::abi; };
struct A { static constexpr int abi = 1; };
struct B {};
template <typename E> struct File {
  int f() requires has_abi<E>;
};
template <typename E> int File<E>::f() requires has_abi<E> { return E::abi; }
template int File<A>::f();
template int File<B>::f();
