// [temp.func.order]/3, /6.2.2, CWG2834 (open): pairs that clang 22 and GCC
// 16.1 order differently, with kiln's answer (docs/status/deviations.md).
// A static member function template against an explicit object one with
// only constraints to tell them apart: GCC lets the constraints decide for
// every object parameter type, clang never does (the parameter lists differ
// in length); kiln keeps its earlier answer, deciding by the constraints
// only when the object parameter is the class by value (`this XS`, as GCC;
// `this XR &` is ambiguous, as clang). An implicit object conversion
// function template against an explicit object one whose object types
// match: GCC lets the constraints decide, clang reports an ambiguity; kiln
// decides, as before. A `const` implicit object member against `this KV` by
// value: clang reports an ambiguity, GCC picks the explicit object one; kiln
// now reports the ambiguity (X(M) is `const KV &`, not `KV`). Found running
// GCC's C++ testsuite (WP-1670).
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 and GCC 16.1 differ on these pairs; kiln's answers are recorded in docs/status/deviations.md
//@ diag: 47:35 error "call to member function 'f' is ambiguous"
//@ diag: 34:7 note "candidate function [with auto:1 = int]"
//@ diag: 35:14 note "candidate function [with auto:1 = int]"
//@ diag: 47:45 error "call to member function 'f' is ambiguous"
//@ diag: 44:7 note "candidate function [with auto:1 = int]"
//@ diag: 45:7 note "candidate function [with auto:1 = int]"
template <class T> concept C = true;
struct XS {
  constexpr int f(this XS, C auto) { return 2; }
  static constexpr int f(auto) { return 3; }
  constexpr int g(this XS, auto) { return 2; }
  static constexpr int g(C auto) { return 3; }
};
static_assert(XS{}.f(0) == 2 && XS{}.g(0) == 3);
struct XR {
  int f(this XR &, C auto) { return 2; }
  static int f(auto) { return 3; }
};
struct CL {
  template <C T> constexpr operator T() const { return 1; }
  template <class T> constexpr operator T(this const CL &) { return 2; }
};
constexpr int r = CL{};
static_assert(r == 1);
struct KV {
  int f(C auto) const { return 1; }
  int f(this KV, auto) { return 2; }
};
int use(XR xr, KV kv) { return xr.f(0) + kv.f(0); }
