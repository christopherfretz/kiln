//@ args: -O1
// [class.dtor]/13, Itanium C++ ABI 5.1.4 (D1/D2): at -O1 clang emits the
// base-object destructor D2 of a class as an alias of its unique base's
// D2 (CodeGenModule::TryEmitBaseDestructorAsAlias) when the body is
// empty, there are no virtual bases and no field needing destruction,
// and exactly one non-virtual base, at offset zero, has a non-trivial
// destructor. A discardable D2 is replaced by the base's; an external one
// aliases it when the base's is already defined and not weak; a weak_odr
// D2 (explicit instantiation, C5/D5 comdat) is always emitted. D1 stays an
// alias of D2 (the optimizer collapses the alias chain).
void ext() noexcept;
struct B { ~B(); int b; };
B::~B() { ext(); }
// Aliased.
struct D : B { ~D(); int d; };
D::~D() {}
// An alias of an alias.
struct DD : D { ~DD(); };
DD::~DD() {}
// Inline: uses are replaced by B's D2.
struct I : B { ~I() {} };
// Internal linkage: also replaced.
namespace { struct A : B { ~A() {} }; }
// A field with a non-trivial destructor.
struct F : B { ~F(); B f; };
F::~F() {}
// Two bases with non-trivial destructors.
struct B2 { ~B2(); };
B2::~B2() { ext(); }
struct T : B, B2 { ~T(); };
T::~T() {}
// The base with a non-trivial destructor at a non-zero offset.
struct E { int e; };
struct O : E, B { ~O(); };
O::~O() {}
// A non-empty body.
struct N : B { ~N(); };
N::~N() { ext(); }
// Virtual destructors: the vtable's D1 slot ends up naming VB's D2.
struct VB { virtual ~VB(); };
VB::~VB() { ext(); }
struct VD : VB { ~VD() override; };
VD::~VD() {}
// A virtual base.
struct VV : virtual B { ~VV(); };
VV::~VV() {}
// The base's D2 is weak_odr (explicit instantiation definition).
template <class X> struct TB { ~TB(); };
template <class X> TB<X>::~TB() { ext(); }
template struct TB<int>;
struct W : TB<int> { ~W(); };
W::~W() {}
// The derived D2 is weak_odr.
template <class X> struct TD : B { ~TD(); };
template <class X> TD<X>::~TD() {}
template struct TD<int>;
// The base's D2 is inline (linkonce_odr).
struct LB { ~LB() { ext(); } };
struct L : LB { ~L(); };
L::~L() {}
// The base's D2 is defined later in the file.
struct LateB { ~LateB(); };
struct Late : LateB { ~Late(); };
Late::~Late() {}
LateB::~LateB() { ext(); }
void use() { I i; A a; D d; DD dd; }
