// WP-847: the operand of a delete-expression ([expr.delete]/1) is emitted
// with its known alignment, as clang `CodeGenFunction::EmitCXXDeleteExpr`
// does through `EmitPointerWithAlignment` (spec/codegen.md §3.3 "Pointer
// alignment"). The alignment reaches every load made through the operand:
// the vtable pointer load of a virtual destructor call ([class.dtor]/16),
// of a destroying operator delete ([expr.delete]/10), and of the
// offset-to-top for `::delete` of a polymorphic object, and the array
// cookie load of `delete[]` ([expr.delete]/12), which is aligned at the
// minimum of the operand's alignment and the cookie's offset from it.
//
// Under oracle 19 the comparison is byte-exact, so the `align N` on those
// loads is checked: `align 128` / `align 256` below for the `alignas`
// globals (the class's natural alignment is 8), and `align 8` for the
// cookie of `&arr[0]` in an `alignas(64)` array (was `align 1`). The
// null test of `delete &g` folds to `false`, as clang's IR builder does.

namespace std {
struct destroying_delete_t {
  explicit destroying_delete_t() = default;
};
}  // namespace std

struct B { virtual ~B(); long x; };
alignas(128) B gb;

void delete_virtual() { delete &gb; }    // vtable load: align 128
void global_delete() { ::delete &gb; }   // offset-to-top, dtor: align 128

struct DD {
  virtual ~DD();
  void operator delete(DD *, std::destroying_delete_t);
};
alignas(256) DD gd;

void destroying_delete() { delete &gd; }   // vtable load: align 256

struct alignas(16) E : B {};
void delete_via_base(E *e) { delete static_cast<B *>(e); }   // align 8

struct S { ~S(); char c; };
alignas(64) S arr[4];

void delete_array_element() { delete[] &arr[0]; }   // cookie: align 8

struct H { long l; S s[4]; };
void delete_member_element(H *h) { delete[] &h->s[1]; }

void delete_param(S *p, B *b) {
  delete[] p;
  delete b;
}
