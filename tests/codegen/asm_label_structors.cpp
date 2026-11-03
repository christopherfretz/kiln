// [dcl.asm] (GNU asm labels): a label on a constructor, destructor or
// conversion function names every structor variant (clang
// `getMangledName` of each `GlobalDecl`): one definition, the base
// variant's; the complete-object alias is skipped because its name is
// defined (`emitConstructorDestructorAlias`). Calls and the vtable's
// complete and deleting slots use the label.
struct S {
  S() asm("ctor_s");
  ~S() asm("dtor_s");
  operator int() asm("conv_s");
  int x;
};
S::S() : x(1) {}
S::~S() {}
S::operator int() { return x; }
struct U { U() asm("ctor_u"); ~U() asm("dtor_u"); operator long() asm("conv_u"); };
struct V { virtual ~V() asm("vdtor"); };
long use() { S s; U u; V v; return s + long(u); }
