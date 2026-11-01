// __has_unique_object_representations(T) ([meta.unary.prop], clang 19
// hasUniqueObjectRepresentations, WP-166): requires a trivially copyable
// type at the top level.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:12 error "incomplete type 'Inc' used in type trait expression"
//@ diag: 12:8 note "forward declaration of 'Inc'"
struct Plain { int x; };
struct NTC { int x; NTC(const NTC&); };
struct NTD { int x; ~NTD(); };
struct Pad { char c; int i; };
struct Inc;
static_assert(__has_unique_object_representations(Plain));
static_assert(!__has_unique_object_representations(NTC));
static_assert(!__has_unique_object_representations(NTD));
static_assert(!__has_unique_object_representations(Pad));
static_assert(!__has_unique_object_representations(NTC[2]));
bool bad = __has_unique_object_representations(Inc);
