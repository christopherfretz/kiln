// [basic.link]/15.1.2, /15.4 (WP-1684): forms GCC 16.1 rejects that the
// text makes valid, kept with clang 22 (docs/status/deviations.md). The
// initializer of a variable or variable template is ignored (a class
// template's static inline data member, a variable template, an inline
// variable holding `&K`), and a default argument that only reads a
// TU-local constant does not odr-use it. clang 22 warns for a call of a
// TU-local function in an inline variable (template)'s initializer; kiln
// gives its warning there.
//@ expect: compile-ok
//@ std: c++20
//@ requires: clang>=22
//@ skip(gcc): GCC 16.1 rejects each but variable_template_not_inline
//@ strict-diags
//@ diag: 24:58 warning "TU local entity 'helper' is exposed [-WTU-local-entity-exposure]"

export module M;
static int helper() { return 1; }
static const int K = 5;

export struct ReadsDefault { int g(int = K); };
export void reads_default(int = K);
export template <class T> struct StaticInline { static inline int v = helper(); };
export inline int use_static_inline() { return StaticInline<int>::v; }
export template <class T> inline int variable_template = helper();
export template <class T> int variable_template_not_inline = helper();
export inline const int *address_of_k = &K;
