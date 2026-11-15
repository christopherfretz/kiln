// [class.cdtor]/3, Itanium C++ ABI §2.6.2-2.6.3: the vtable group, VTT and construction vtables of
// a class whose virtual base sits at different offsets in it (C), with it as a non-virtual base
// (E) and with that as a virtual base (F), so a base-subobject constructor must read the virtual
// base offset from the construction vtable. Found running GCC's C++ testsuite.
//@ expect: vtables
struct V { int v = 7; virtual const char* who(); virtual ~V(); };
struct Sink { V* got; Sink(V* p) : got(p) {} };
struct C : Sink, virtual V { int pad[3]; C(); const char* who() override; };
struct Pad { long long q[5]; };
struct E : Pad, C { const char* who() override; };
struct F : Pad, virtual E { int f; const char* who() override; };
V::~V() {}
const char* V::who() { return "V"; }
C::C() : Sink(this) {}
const char* C::who() { return "C"; }
const char* E::who() { return "E"; }
const char* F::who() { return "F"; }
F make() { return F(); }
