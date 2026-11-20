// [dcl.attr.grammar]/4: an attribute followed by `...` is an attribute
// pack expansion, valid only where the attribute's specification allows
// one, and no attribute's does: reported at the token after the `...`
// (clang's err_cxx11_attribute_forbids_ellipsis), the attribute kept or
// ignored as without it. An argument naming the pack is unexpanded
// ([temp.variadic]/12), reported at the definition. Reduced from GCC's
// g++.dg/cpp0x/alignas4.C and gen-attrs-65.C, which GCC accepts as an
// extension.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++26
//@ diag: 19:18 error "expression contains unexpanded parameter pack 'T'"
//@ diag: 19:32 error "attribute 'aligned' cannot be used as an attribute pack"
//@ diag: 22:26 error "attribute 'foobar' cannot be used as an attribute pack"
//@ diag: 22:5 warning "unknown attribute 'foobar' ignored"
//@ diag: 25:17 error "attribute 'nodiscard' cannot be used as an attribute pack"
//@ diag: 26:23 error "attribute 'deprecated' cannot be used as an attribute pack"
template <class... T> struct A2 {
  [[gnu::aligned(alignof(T))...]] char t;
};
template <class... T> struct S {
  [[foobar(alignof(T))...]] char t;
};
struct N {
  [[nodiscard...]] int f();
  [[deprecated("x")...]] int g();
};
// alignas takes a pack expansion ([dcl.align]/4).
template <class... T> struct A1 { alignas(T...) char t; };
static_assert(alignof(A1<char, double>) == alignof(double));
