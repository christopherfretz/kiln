// An ambiguous derived-to-base conversion is diagnosed at the converted
// operand, naming the class types with their cv-qualifiers, as clang 22
// does (`CheckPointerConversion`, `CheckDerivedToBaseConversion`): in an
// initialization, a C-style cast (the static_cast-then-const_cast
// reading, [expr.cast]/4.3), a reference binding and a static_cast; a
// reference cast names the derived class with the referent's qualifiers.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -Wno-inaccessible-base
//@ strict-diags
//@ diag: 23:16 error "ambiguous conversion from derived class 'const F' to base class 'const V':"
//@ diag: 24:13 error "ambiguous conversion from derived class 'const F' to base class 'V':"
//@ diag: 25:19 error "ambiguous conversion from derived class 'volatile F' to base class 'const V':"
//@ diag: 26:16 error "ambiguous conversion from derived class 'const F' to base class 'const V':"
//@ diag: 27:31 error "ambiguous conversion from derived class 'const F' to base class 'const V':"
//@ diag: 30:25 error "ambiguous conversion from derived class 'F' to base class 'V':"
//@ diag: 31:13 error "ambiguous conversion from derived class 'F' to base class 'V':"
//@ diag: 32:19 error "ambiguous conversion from derived class 'const F' to base class 'const V':"
struct V { int v; };
struct E : virtual V {};
struct F : V, E {};
void h(const F* f, volatile F* g, const F& rf) {
  const V* a = f;
  (void)(V*)f;
  (void)(const V*)g;
  const V& b = rf;
  (void)static_cast<const V*>(f);
}
void k(F& ff, const F& rf) {
  (void)static_cast<V&>(ff);
  (void)(V&)rf;
  (void)(const V&)ff;
}
