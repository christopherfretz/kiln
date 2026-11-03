//@ args: -O1
// TBAA on vtable pointer accesses at -O1 (Itanium C++ ABI §2.5.2, [class.virtual]):
// a constructor's vptr stores and every vptr load (a virtual call,
// `dynamic_cast` to `void*`, `typeid` of a polymorphic glvalue, a virtual
// base offset, a virtual member-function-pointer call, `::delete` of a
// polymorphic object, a virtual thunk's this-adjustment) are tagged
// "vtable pointer", a child of the root. The `this` spill store is
// tagged with the class pointer type.
#define ON __attribute__((optnone))
namespace std { class type_info; }
struct A { int a; ON A(); virtual ~A(); virtual int f(); };
ON A::A() : a(1) {}
struct B : virtual A { int b; ON B(); ~B() override; };
ON B::B() : b(2) {}
ON B::~B() {}
ON int vcall(A* a) { return a->f(); }
ON const void* to_void(A* a) { return dynamic_cast<const void*>(a); }
ON const std::type_info* type_of(A* a) { return &typeid(*a); }
ON int vbase(B* b) { return b->a; }
ON int pmf_call(A* a, int (A::*m)()) { return (a->*m)(); }
ON void global_delete(A* a) { ::delete a; }
