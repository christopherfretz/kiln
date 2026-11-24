// new- and delete-expressions in the written-reference table
// (spec/ast.md §18.13.6, stage 6c): the `new` keyword names the
// allocation function the new-expression selected ([expr.new]/12: the
// class's scope for a class type, then the global scope; `::new` the
// global scope only), the `delete` keyword (`delete[]` one entry over its
// three tokens) the deallocation function ([expr.delete]/9-11: usual,
// sized, aligned, destroying), `how=qualified`. `::new` and `::delete`
// have qualifier GLOBAL with the `::` as lead. The global replaceable
// functions ([basic.stc.dynamic.general]/2) are functions like any other;
// an allocation function template's specialization has `found=` the
// template. Under a virtual destructor the entry names what the static
// type's lookup selected. The deallocation function called if the
// initialization throws and the destructor a delete calls have no token:
// no entry. An explicit `::operator new(n)` is a name (role `expr`).
//@ std: c++20
//@ written-refs: operators-new-delete.refs
using size_t = decltype(sizeof 0);
namespace std {
enum class align_val_t : size_t {};
struct destroying_delete_t {
  explicit destroying_delete_t() = default;
};
} // namespace std
void *operator new(size_t, void *p) noexcept;
struct Arena {};
template <class T> void *operator new(size_t, T &);

struct C {
  static void *operator new(size_t);
  static void *operator new[](size_t);
  static void operator delete(void *);
  static void operator delete[](void *);
};
struct Sized {
  void operator delete(void *, size_t);
  ~Sized();
};
struct alignas(64) Over {
  static void *operator new(size_t, std::align_val_t);
  static void operator delete(void *, std::align_val_t);
};
struct D {
  void operator delete(D *, std::destroying_delete_t);
};
struct Base {
  virtual ~Base();
  void operator delete(void *);
};
struct Derived : Base {
  ~Derived() override;
  void operator delete(void *);
};

void use(void *buf, Base *bp, Arena &a) {
  int *i = new int;
  delete i;
  int *ia = new int[4];
  delete[] ia;
  C *c = new C;
  delete c;
  C *ca = new C[2];
  delete [] ca;
  C *g = ::new C;
  ::delete g;
  C *ga = ::new C[2];
  ::delete[] ga;
  int *p = new (buf) int(3);
  Sized *s = new Sized;
  delete s;
  Over *o = new Over;
  delete o;
  D *d = new D;
  delete d;
  delete bp;
  Derived *dp = new Derived;
  delete dp;
  int *t = new (a) int;
  static_assert(requires { new C; });
  void *raw = ::operator new(4);
  (void)i, (void)p, (void)t, (void)raw;
}

template <class T> int *in_template(T) {
  // Not dependent: resolved at the definition, no rows.
  return new int;
}
int *inst = in_template(1);
