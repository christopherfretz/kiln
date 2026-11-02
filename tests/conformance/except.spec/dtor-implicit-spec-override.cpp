// [except.spec]/8, /11, [class.virtual]: a destructor declared without an
// exception specification has the implicit one, noexcept unless a
// subobject's destructor is potentially-throwing, also when it is
// defined out of line or in a template, so it may override a noexcept
// virtual destructor; a potentially-throwing implicit one may not.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:21 error "exception specification of overriding function is more lax than base version"
//@ diag: 12:20 note "overridden virtual function is here"
//@ diag: 21:8 error "exception specification of overriding function is more lax than base version"
//@ diag: 12:20 note "overridden virtual function is here"
struct B { virtual ~B(); };
struct D : B { ~D() override; };
D::~D() {}
struct M { ~M() noexcept(false); };
struct E : B { M m; ~E() override; };
E::~E() {}
template <class C> struct T : B { ~T() override; };
template <class C> T<C>::~T() {}
template struct T<char>;
struct F : B { M m; };
struct G { ~G(); };
struct H : B { G g; ~H() override; };
H::~H() {}
