//@ args: -Oz
// [expr.delete] at -Oz (clang `EmitObjectDelete`, `OptimizeSize` 2): a
// non-array delete-expression whose deallocation is not done by a virtual
// deleting destructor or a destroying operator delete calls `operator
// delete` unconditionally, after the null check's merge block (the
// destructor, if any, still runs only for a non-null pointer). Array
// deletes, virtual deleting destructors and destroying deletes keep the
// conditional form (spec/codegen.md "Size optimization attributes").
namespace std {
struct destroying_delete_t {
  explicit destroying_delete_t() = default;
};
} // namespace std
struct E {};
struct N {
  ~N();
};
struct V {
  virtual ~V();
};
struct Dd {
  void operator delete(Dd *, std::destroying_delete_t);
};
void d1(E *p) { delete p; }
void d2(N *p) { delete p; }
void d3(V *p) { delete p; }
void d4(Dd *p) { delete p; }
void d5(E *p) { delete[] p; }
void d6(N *p) { delete[] p; }
void d7(V *p) { ::delete p; }
