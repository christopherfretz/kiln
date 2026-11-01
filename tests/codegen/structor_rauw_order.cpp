// A discardable complete-object structor is replaced by its base-object
// variant (clang `StructorCodegen::RAUW`); `applyReplacements` moves the
// base function to where the complete-object declaration was created.
// When the base variant already exists (called by a derived class's
// structor) and the complete variant is first referenced later (a
// vtable emitted at the end of the TU), the base function moves there.

template <class T> struct A { ~A() {} };
template <class T> struct B : A<T> { ~B() {} void f(); };
template <class T> void B<T>::f() { A<T> a; }
template struct B<int>;

struct VB { virtual ~VB() {} };
template <class T> struct D : VB { ~D() {} D() {} };
template struct D<int>;

template <class T> struct VT { virtual ~VT() {} };
template <class T> struct E : VT<T> { E() {} ~E() {} };
template struct E<int>;

template <class T> struct VV { virtual ~VV() {} };
template <class T> struct F : virtual VV<T> { ~F() {} F() {} };
template struct F<int>;

int z() { return 1; }
