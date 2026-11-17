// [expr.prim.this]/3: `this` may not appear in the declaration of a static
// member function, so a non-static member function named without an object
// in its trailing return type has no object ([expr.prim.id.general]/2):
// substituting `g<int>` fails and the call has no candidate. kiln
// substitutes a static member function's signature without `this` in
// scope, so the reason is the call without an object (GCC 16.1 says the
// same); clang 22 substitutes with `this` and its reason is that the
// template argument is not a constant expression, so only the common part
// of the note is matched. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++20
//@ diag: 21:9 error "no matching function for call to 'g'"
//@ diag: 18:34 note "candidate template ignored: substitution failure [with T = int]: "
template <bool> struct Tag {};
struct Holder {
  template <class> constexpr bool is() { return true; }
  template <class T> static auto g() -> Tag<is<T>()>;
};
void use() {
  (void)Holder::g<int>();
}
