// [basic.stc.dynamic.allocation], [expr.new]/19, [expr.delete]/10 (P2719,
// WP-1112): a std::type_identity that is a class but not a class template
// still makes `operator new`/`delete` type-aware declarations, which are
// unusable: each use that would build `std::type_identity<T>` is
// diagnosed at the malformed declaration (clang `LookupStdClassTemplate`,
// not remembered), and the expression falls back to the untyped functions.
// A namespace-scope type-aware deallocation function is not found at all.
//@ verify
//@ requires: clang>=22
//@ args: -ferror-limit=0 -Wno-ext-cxx-type-aware-allocators
namespace std {
  struct type_identity {}; // #ti
  using size_t = __SIZE_TYPE__;
  enum class align_val_t : long {};
}

void *operator new(std::type_identity, std::size_t, std::align_val_t);
void operator delete(std::type_identity, void *, std::size_t, std::align_val_t);

struct T {};
void global() {
  T *p = new T; // the untyped global operator new
  // expected-error@#ti {{std::type_identity must be a class template with a single type parameter}}
  delete p;
  new T[2];
  // expected-error@#ti {{std::type_identity must be a class template with a single type parameter}}
}

// Class-scope: the member functions are found, but not usable.
struct S {
  template <class U> void *operator new(std::type_identity, std::size_t, std::align_val_t);
  template <class U> void operator delete(std::type_identity, void *, std::size_t, std::align_val_t); // #sdel
};
void member(S *p) {
  delete p; // expected-error {{no suitable member 'operator delete' in 'S'}}
  // expected-note@#sdel {{member 'operator delete' declared here}}
  // expected-error@#ti {{std::type_identity must be a class template with a single type parameter}}
  new S; // expected-error {{no matching function for call to 'operator new'}}
  // expected-error@#ti {{std::type_identity must be a class template with a single type parameter}}
}

// A virtual destructor's deallocation function.
struct V {
  virtual ~V();
  void operator delete(std::type_identity, void *, std::size_t, std::align_val_t); // #vdel
  void *operator new(std::type_identity, std::size_t, std::align_val_t);
};
V::~V() {} // expected-error {{no suitable member 'operator delete' in 'V'}}
// expected-note@#vdel {{member 'operator delete' declared here}}
// expected-error@#ti {{std::type_identity must be a class template with a single type parameter}}
