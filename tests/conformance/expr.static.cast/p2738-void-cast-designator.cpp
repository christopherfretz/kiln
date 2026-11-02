// [expr.static.cast]/14, [expr.const]/5.14 (C++26, P2738R1), [conv.qual]/2:
// a cast from `void *` to `T *` is constant only when the object the
// `void *` designates (the subobject its conversion from `U *` named, not
// any object at that address) has a type similar to `T`: a base subobject
// is not the derived object, a first member is not the enclosing object,
// an empty base and a `[[no_unique_address]]` member at one offset are told
// apart, an array is not its first element, and the designator survives a
// store of the `void *` to memory. Similar types (cv-qualifiers at any
// level) and one-past-the-end pointers are accepted.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 48:15 error "static assertion expression is not an integral constant expression"
//@ diag: 47:87 note "cast from 'void *' is not allowed in a constant expression because the pointed object type 'B' is not similar to the target type 'D'"
//@ diag: 48:15 note "in call to 'base_as_derived()'"
//@ diag: 50:15 error "static assertion expression is not an integral constant expression"
//@ diag: 49:69 note "cast from 'void *' is not allowed in a constant expression because the pointed object type 'D' is not similar to the target type 'B'"
//@ diag: 50:15 note "in call to 'derived_as_base()'"
//@ diag: 52:15 error "static assertion expression is not an integral constant expression"
//@ diag: 51:75 note "cast from 'void *' is not allowed in a constant expression because the pointed object type 'int' is not similar to the target type 'D'"
//@ diag: 52:15 note "in call to 'member_as_enclosing()'"
//@ diag: 54:15 error "static assertion expression is not an integral constant expression"
//@ diag: 53:75 note "cast from 'void *' is not allowed in a constant expression because the pointed object type 'F' is not similar to the target type 'E'"
//@ diag: 54:15 note "in call to 'nua_member_as_empty_base()'"
//@ diag: 56:15 error "static assertion expression is not an integral constant expression"
//@ diag: 55:91 note "cast from 'void *' is not allowed in a constant expression because the pointed object type 'E' is not similar to the target type 'F'"
//@ diag: 56:15 note "in call to 'empty_base_as_nua_member()'"
//@ diag: 58:15 error "static assertion expression is not an integral constant expression"
//@ diag: 57:80 note "cast from 'void *' is not allowed in a constant expression because the pointed object type 'int[3]' is not similar to the target type 'int'"
//@ diag: 58:15 note "in call to 'array_as_element()'"
//@ diag: 60:15 error "static assertion expression is not an integral constant expression"
//@ diag: 59:80 note "cast from 'void *' is not allowed in a constant expression because the pointed object type 'int' is not similar to the target type 'int[3]'"
//@ diag: 60:15 note "in call to 'element_as_array()'"
//@ diag: 62:15 error "static assertion expression is not an integral constant expression"
//@ diag: 61:81 note "cast from 'void *' is not allowed in a constant expression because the pointed object type 'B' is not similar to the target type 'D'"
//@ diag: 62:15 note "in call to 'through_member()'"
//@ diag: 64:15 error "static assertion expression is not an integral constant expression"
//@ diag: 63:101 note "cast from 'void *' is not allowed in a constant expression because the pointed object type 'int' is not similar to the target type 'D'"
//@ diag: 64:15 note "in call to 'through_copy()'"
struct B { int b; };
struct D : B { int d; };
struct E {};
struct F {};
struct S : E { [[no_unique_address]] F f; int x; };
struct H { void *p; };

// Not similar.
constexpr int base_as_derived() { D d{{1}, 2}; void *p = static_cast<B *>(&d); return static_cast<D *>(p)->d; }
static_assert(base_as_derived() == 2);
constexpr int derived_as_base() { D d{{1}, 2}; void *p = &d; return static_cast<B *>(p)->b; }
static_assert(derived_as_base() == 1);
constexpr int member_as_enclosing() { D d{{1}, 2}; void *p = &d.b; return static_cast<D *>(p)->d; }
static_assert(member_as_enclosing() == 2);
constexpr bool nua_member_as_empty_base() { S s{}; void *p = &s.f; return static_cast<E *>(p) != nullptr; }
static_assert(nua_member_as_empty_base());
constexpr bool empty_base_as_nua_member() { S s{}; void *p = static_cast<E *>(&s); return static_cast<F *>(p) != nullptr; }
static_assert(empty_base_as_nua_member());
constexpr int array_as_element() { int a[3] = {1, 2, 3}; void *p = &a; return *static_cast<int *>(p); }
static_assert(array_as_element() == 1);
constexpr int element_as_array() { int a[3] = {1, 2, 3}; void *p = a; return (*static_cast<int(*)[3]>(p))[1]; }
static_assert(element_as_array() == 2);
constexpr int through_member() { D d{{1}, 2}; H h{static_cast<B *>(&d)}; return static_cast<D *>(h.p)->d; }
static_assert(through_member() == 2);
constexpr int through_copy() { D d{{1}, 2}; H h{&d.b}; H k = h; void *a[2] = {k.p, nullptr}; return static_cast<D *>(a[0])->d; }
static_assert(through_copy() == 2);

// Similar.
constexpr bool nua_member() { S s{}; void *p = &s.f; return static_cast<F *>(p) == &s.f; }
static_assert(nua_member());
constexpr bool empty_base() { S s{}; void *p = static_cast<E *>(&s); return static_cast<E *>(p) == static_cast<E *>(&s); }
static_assert(empty_base());
constexpr int cv() { int x = 4; const void *p = &x; return *static_cast<const int *>(const_cast<void *>(p)); }
static_assert(cv() == 4);
constexpr int past_end() { int a[3] = {1, 2, 3}; void *p = a + 3; return static_cast<int *>(p)[-1]; }
static_assert(past_end() == 3);
constexpr int nested_cv() { int x = 4; int *q = &x; void *p = &q; return **static_cast<const int *const *>(p); }
static_assert(nested_cv() == 4);
constexpr int round_trip() { D d{{1}, 2}; H h{&d}; void *l = h.p; void **pl = &l; return static_cast<D *>(*pl)->d; }
static_assert(round_trip() == 2);
constexpr int null_ptr() { void *p = nullptr; H h{p}; return static_cast<D *>(h.p) == nullptr; }
static_assert(null_ptr() == 1);
