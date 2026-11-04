// [class.abstract]/2, [class.dtor]/13: a class without a declared
// destructor gets an implicit one, which overrides a base's pure virtual
// destructor and is not pure, so the class is not abstract
// (boost::wrapexcept over boost::exception's `virtual ~exception() = 0`).
//@ strict-diags
struct B { virtual ~B() = 0; };
inline B::~B() {}
struct D : B {};
D *f() { return new D; }
struct E : B { virtual void g() {} };
E *h() { return new E; }
template <class X> struct W : X, B { W *clone() const { return new W(*this); } };
struct X { virtual ~X() {} };
W<X> *g() { return W<X>().clone(); }
static_assert(__is_abstract(B) && !__is_abstract(D) && !__is_abstract(W<X>));
