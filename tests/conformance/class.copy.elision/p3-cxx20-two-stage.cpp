// [class.copy.elision]/3 (C++20): the operand of a return statement that
// names an implicitly movable entity is first treated as an rvalue; when
// that initialization fails (other than by selecting a deleted function)
// it is performed again with the operand as an lvalue. So an rvalue
// reference parameter returned as `T&` binds as the lvalue it names
// (C++23's P2266 rejects it). A selected deleted move constructor is still
// an error. `decltype(auto)` of `return (x);` is `T&` before C++23.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 23:30 error "call to deleted constructor of 'D'"
//@ diag: 22:30 note "'D' has been explicitly marked deleted here"
template <class T> constexpr T& as_lvalue(T&& t) { return t; }
struct S { int x; };
int& from_int(int&& t) { return t; }
S& from_class(S&& t) { return t; }
const S& to_const(S&& t) { return t; }
// No move constructor and a copy constructor taking `C&`: the rvalue
// attempt finds no viable constructor, the lvalue one succeeds.
struct C { C(); C(C&); };
C copy_from_nonconst(C c) { return c; }
struct D { D(); D(const D&); D(D&&) = delete; };
D deleted_move(D d) { return d; }
decltype(auto) paren(int&& x) { return (x); }
static_assert(__is_same(decltype(paren(1)), int&));
int use() {
  S s{1};
  return as_lvalue(S{2}).x + from_class(static_cast<S&&>(s)).x + from_int(3);
}
