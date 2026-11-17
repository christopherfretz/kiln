// [expr.prim.this]/3: `this` may not appear in the declaration of a static
// member function, so a non-static member function named without an object
// in its trailing return type has no object ([expr.ref]); P2280R4's
// unknown `this` ([expr.const]/10.1) does not apply. Substituting `g<int>`
// fails and the call has no candidate. kiln's sema enters a `this` scope
// for static member declarators (decl.rs act_on_start_this_scope callers),
// so after WP-1527 the Oven evaluates `is<T>()` with an unknown `this`.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++20
//@ xfail(kiln): WP-1572: implicit this in a static member function template's trailing return type is accepted
//@ diag: 21:9 error "no matching function for call to 'g'"
//@ diag: 18:34 note "candidate template ignored: substitution failure [with T = int]: non-type template argument is not a constant expression"
template <bool> struct Tag {};
struct Holder {
  template <class> constexpr bool is() { return true; }
  template <class T> static auto g() -> Tag<is<T>()>;
};
void use() {
  (void)Holder::g<int>();
}
