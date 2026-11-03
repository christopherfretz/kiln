// [class.dtor]/7.3, /16: a virtual destructor looks up the deallocation
// function its deleting destructor calls from the class's scope; a
// user-provided one checks its access at the destructor (clang
// `CheckDestructor`), and an implicit or defaulted one is defined as
// deleted when that function is ambiguous or inaccessible — overriding a
// non-deleted destructor is then ill-formed ([class.virtual]/18). A
// destroying operator delete participates like a usual one.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:8 error "deleted function '~W' cannot override a non-deleted function"
//@ diag: 27:20 note "overridden virtual function is here"
//@ diag: 29:8 note "virtual destructor requires an unambiguous, accessible 'operator delete'"
//@ diag: 30:3 error "attempt to use a deleted function"
//@ diag: 29:8 note "virtual destructor requires an unambiguous, accessible 'operator delete'"
//@ diag: 31:16 error "'operator delete' is a private member of 'V'"
//@ diag: 27:40 note "declared private here"
//@ diag: 35:16 error "'operator delete' is a protected member of 'P'"
//@ diag: 32:28 note "declared protected here"
//@ diag: 37:8 error "deleted function '~DW' cannot override a non-deleted function"
//@ diag: 36:21 note "overridden virtual function is here"
//@ diag: 37:8 note "virtual destructor requires an unambiguous, accessible 'operator delete'"
//@ diag: 39:5 error "'operator delete' is a private member of 'DV'"
//@ diag: 36:42 note "declared private here"
namespace std {
struct destroying_delete_t { explicit destroying_delete_t() = default; };
}
struct V { virtual ~V(); private: void operator delete(void *); };
V::~V() {}
struct W : V {};
W w;
struct X : V { ~X() override {} };
struct P { protected: void operator delete(void *); public: virtual ~P(); };
struct Q : P { ~Q() override; };
Q::~Q() {}
void g(Q *q) { delete q; }
struct DV { virtual ~DV(); private: void operator delete(DV *, std::destroying_delete_t); };
struct DW : DV {};
struct DX : DV { ~DX() override; };
DX::~DX() {}
