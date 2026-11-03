// [dcl.init.ref]/5 [ACLE] An SVE vector element, like a GNU vector element,
// cannot bind a non-const lvalue reference (clang
// `err_reference_bind_to_vector_element`; a `const` reference binds a
// temporary copy). clang's `refersToVectorElement` tests `isVectorType`, which
// the SVE builtin types are not, so clang accepts this and binds the
// reference to the vector's address, dropping the index; kiln keeps the
// GNU vector rule. spec/sema.md "Vector subscript".
//@ skip(clang): clang accepts the binding and miscompiles it (the index is dropped)
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sve
//@ strict-diags
//@ diag: 17:8 error "non-const reference cannot bind to vector element"

typedef __SVInt32_t i32;

int f(i32 &v, int a) {
  int &r = v[a];
  const int &c = v[a];
  return r + c;
}
