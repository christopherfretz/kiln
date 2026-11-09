// [expr.new]/20, [over.match.best] (P2719, WP-1112): a type-aware
// allocation function is selected by overload resolution like any other;
// an ambiguous best candidate is an error naming the operator, with the
// tied candidates, and does not fall back to the untyped functions
// (clang `resolveAllocationOverload`, `err_ovl_ambiguous_call`).
//@ verify
//@ requires: clang>=22
//@ args: -ferror-limit=0 -Wno-ext-cxx-type-aware-allocators
namespace std {
  template <class T> struct type_identity {};
  using size_t = __SIZE_TYPE__;
  enum class align_val_t : size_t {};
}
using size_t = __SIZE_TYPE__;

// Global type-aware templates, tied on the placement argument.
template <class T> void *operator new(std::type_identity<T>, size_t, std::align_val_t, int); // #g1
template <class T> void *operator new(std::type_identity<T>, size_t, std::align_val_t, long); // #g2
template <class T> void operator delete(std::type_identity<T>, void *, size_t, std::align_val_t);
template <class T> void *operator new[](std::type_identity<T>, size_t, std::align_val_t, int); // #a1
template <class T> void *operator new[](std::type_identity<T>, size_t, std::align_val_t, long); // #a2
template <class T> void operator delete[](std::type_identity<T>, void *, size_t, std::align_val_t);
// The untyped forms the ambiguity does not fall back to.
void *operator new(size_t, double);
void *operator new[](size_t, double);

struct G {};
void global() {
  new (1.0) G; // expected-error {{call to 'operator new' is ambiguous}}
  // expected-note@#g1 {{candidate function [with T = G]}}
  // expected-note@#g2 {{candidate function [with T = G]}}
  new (1.0) G[2]; // expected-error {{call to 'operator new[]' is ambiguous}}
  // expected-note@#a1 {{candidate function [with T = G]}}
  // expected-note@#a2 {{candidate function [with T = G]}}
}

template <class U> void instantiated() {
  new (1.0) U; // expected-error {{call to 'operator new' is ambiguous}}
  // expected-note@#g1 {{candidate function [with T = G]}}
  // expected-note@#g2 {{candidate function [with T = G]}}
}
template void instantiated<G>(); // expected-note {{in instantiation of function template specialization 'instantiated<G>' requested here}}

// Class-scope type-aware templates.
struct C {
  template <class T> void *operator new(std::type_identity<T>, size_t, std::align_val_t, int); // #c1
  template <class T> void *operator new(std::type_identity<T>, size_t, std::align_val_t, long); // #c2
  template <class T> void operator delete(std::type_identity<T>, void *, size_t, std::align_val_t);
  void *operator new(size_t, double);
};
void class_scope() {
  new (1.0) C; // expected-error {{call to 'operator new' is ambiguous}}
  // expected-note@#c1 {{candidate function [with T = C]}}
  // expected-note@#c2 {{candidate function [with T = C]}}
}

// Class-scope type-aware non-templates.
struct N {
  void *operator new(std::type_identity<N>, size_t, std::align_val_t, int); // #n1
  void *operator new(std::type_identity<N>, size_t, std::align_val_t, long); // #n2
  void operator delete(std::type_identity<N>, void *, size_t, std::align_val_t);
};
void non_template() {
  new (1.0) N; // expected-error {{call to 'operator new' is ambiguous}}
  // expected-note@#n1 {{candidate function}}
  // expected-note@#n2 {{candidate function}}
}

// Typed vs untyped with equal conversions: the type-aware set is tried
// first, so its viable candidate wins, no ambiguity.
struct Tie {
  template <class T> void *operator new(std::type_identity<T>, size_t, std::align_val_t);
  template <class T> void operator delete(std::type_identity<T>, void *, size_t, std::align_val_t);
  void *operator new(size_t);
  void operator delete(void *);
};
void tie() { delete new Tie; }

// A non-template type-aware function is better than templates matching
// as well; a variadic template is worse: no ambiguity.
template <class T> void *operator new(std::type_identity<T>, size_t, std::align_val_t);
template <class T> void *operator new(std::type_identity<T>, size_t, std::align_val_t, ...);
void *operator new(std::type_identity<int>, size_t, std::align_val_t);
void operator delete(std::type_identity<int>, void *, size_t, std::align_val_t);
void ordered() { delete new int; }
