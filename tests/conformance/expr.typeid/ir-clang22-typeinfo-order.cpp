// [expr.typeid]/1, [except.throw]/3, [class.virtual]: the `type_info`
// objects of clang 22 at -fclang-compat=22 (spec/codegen.md §6a): each
// `_ZTI` global is created before its `_ZTS` name, its `__cxxabiv1`
// vtable and the descriptors of its pointee or bases, so it is printed
// first (clang 19 creates it after them). Compared with clang 22's IR
// exactly.
//@ expect: ir
//@ requires: clang>=22
namespace std { class type_info; }
struct L { int x; };
struct A { virtual void f(); };
void A::f() {}
struct B : A { void f() override; };
void B::f() {}
struct M1 { virtual ~M1(); };
struct M2 { virtual ~M2(); };
struct MI : M1, M2 { ~MI(); };
MI::~MI() {}
struct Thrown { virtual void g() {} };

const void *ptr() { return &typeid(L *); }
const void *ptr2() { return &typeid(const L **); }
const void *member_ptr() { return &typeid(int L::*); }
const void *noexcept_fn() { return &typeid(void (*)() noexcept); }
void throw_it() { throw Thrown{}; }
