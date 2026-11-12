// [expr.const.const]/2, /5.1 and [expr.const.imm]/2.1-3 (P4101R1): every
// subobject of an object holding a non-null reflection is an immediate
// object, so a glvalue naming it (`t.label`, the whole `t`) is an
// immediate constant expression and is immediate-escalating; outside an
// immediate-escalating function it is an error, whether the variable is
// a constexpr local, a namespace-scope one, an expansion statement's
// element (by value or by reference), a non-constexpr copy's initializer
// in an expansion, or a constexpr local of a function template
// specialization. The same holds for binding the element to a reference
// parameter. The well-formed spelling reads the member in a `constexpr`
// initializer first (p5-immediate-glvalue-run.cpp). Compared with GCC
// 16.1, which rejects all of these under the P2996R13 type rule except
// the non-constexpr copy (accepted there; see docs/status/deviations.md).
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection
//@ skip(clang): clang 19/22 do not implement P2996 reflection
//@ strict-diags
//@ diag: 35:36 error "consteval-only value outside an immediate function context"
//@ diag: 39:8 error "consteval-only value outside an immediate function context"
//@ diag: 43:47 error "consteval-only value outside an immediate function context"
//@ diag: 44:54 error "consteval-only value outside an immediate function context"
//@ diag: 46:24 error "consteval-only value outside an immediate function context"
//@ diag: 51:6 error "function whose type holds 'std::meta::info' must be declared 'consteval' (a run-time 'std::meta::info' is not supported yet)"
//@ diag: 53:47 error "consteval-only value outside an immediate function context"
//@ diag: 58:8 error "consteval-only value outside an immediate function context"
//@ diag: 60:15 note "in instantiation of function template specialization 'in_template<int>' requested here"
extern "C" int puts(const char*);
using info = decltype(^^int);
struct Tag { info what; const char* label; };

constexpr Tag tags[] = {{^^char, "char"}, {^^short, "short"}};
constexpr Tag one{^^bool, "bool"};

const char* from_global() { return one.label; }        // a namespace-scope variable

void plain() {
  constexpr Tag t{^^float, "float"};
  puts(t.label);                                       // a constexpr local
}

void expansion() {
  template for (constexpr auto t : tags) puts(t.label);           // by value
  template for (constexpr const auto& t : tags) puts(t.label);    // by reference
  template for (constexpr auto t : tags) {
    const char* copy = t.label;                                   // a non-constexpr copy
    puts(copy);
  }
}

void take(const Tag&);
void bind() {
  template for (constexpr auto t : tags) take(t);                 // reference binding
}

template <class T> void in_template() {
  constexpr Tag t{^^T, "T"};
  puts(t.label);
}
template void in_template<int>();
