// [expr.const.imm]/2 (P4101R1) Examples 1 and 2. /2.1: `F()` of
// `ex<refl>` is an immediate constant expression (its result object
// holds a non-null reflection), so the constexpr function template
// specialization `ex<refl>` is an immediate function and taking its
// address is an error. /2.2: `F(i)` of `apply_to<id>` is an immediate
// invocation that is not a constant expression. Both specializations are
// formed (a pointer to an immediate function is a valid template
// argument: its template parameter object is an immediate object,
// [expr.const.const]/3.1, carrying P3603). kiln's reason note names the
// consteval-only value `F` (a template parameter object, a leaf), not
// the invocation. `ex2` checks /2.1 itself (/2.2 is `expr.const/`
// territory, unchanged by P4101R1). Compared with GCC 16.1 (the verdict;
// it rejects the same lines).
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection
//@ skip(clang): clang 19/22 do not implement P2996 reflection
//@ strict-diags
//@ diag: 34:11 error "cannot take address of immediate function 'ex<&refl>' outside of an immediate invocation"
//@ diag: 31:16 note "declared here"
//@ diag: 32:12 note "'ex<&refl>' is an immediate function because its body contains a consteval-only value of type 'info (*)()' (aka 'std::meta::info (*)()')"
//@ diag: 40:12 error "cannot take address of immediate function 'apply_to<&id>' outside of an immediate invocation"
//@ diag: 39:16 note "declared here"
//@ diag: 39:41 note "'apply_to<&id>' is an immediate function because its body contains a consteval-only value of type 'int (*)(int)'"
//@ diag: 50:16 error "cannot take address of immediate function 'ex2<int>' outside of an immediate invocation"
//@ diag: 47:16 note "declared here"
//@ diag: 48:12 note "'ex2<int>' is an immediate function because its body contains a consteval-only value of type 'info' (aka 'std::meta::info')"
using info = decltype(^^int);
consteval info refl() { return ^^int; }
template <auto F>
constexpr void ex() {
  auto x = F();
}
auto p = &ex<refl>; // error: immediate function because F()
                    // is an immediate constant expression

consteval int id(int x) { return x; }
template <auto F>
constexpr auto apply_to(int i) { return F(i); }
auto p2 = &apply_to<id>; // error: immediate function because F(i)
                         // is not a constant expression


// /2.1 without a function-pointer template argument: the specialization
// is formed and the reason is noted.
template <class T>
constexpr void ex2() {
  auto x = refl(); // /2.1: escalates even though `refl()` is a constant
}
void (*q)() = &ex2<int>;
