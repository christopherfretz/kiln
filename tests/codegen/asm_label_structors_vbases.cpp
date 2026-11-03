// [dcl.asm] (GNU asm labels) on the structors of classes with virtual
// bases (WP-492): every variant takes the label, so clang's base-object
// definition (with the VTT parameter) is replaced by the complete-object
// one at the end of the module (`GetOrCreateLLVMFunction` for a
// definition of another type), which emits the class's vtable and VTT.
// A labelled virtual destructor's deleting variant is appended to the
// function as unreachable blocks (`; No predecessors!`).
struct B { int b; };
struct D : virtual B {
  D() asm("ctor_d");
  ~D() asm("dtor_d");
};
D::D() {}
D::~D() {}
struct E : D { E(); ~E(); };
E::E() {}
E::~E() {}
struct V { virtual ~V() asm("vdtor"); int v; };
V::~V() {}
struct W : virtual B { virtual ~W() asm("vdtor_w"); };
W::~W() {}
int use() { D d; E e; W w; return 0; }
