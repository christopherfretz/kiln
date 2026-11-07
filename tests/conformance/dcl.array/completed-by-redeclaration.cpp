// [basic.types.general]/6, [dcl.array]/8, [dcl.type.decltype]/1,
// [expr.sizeof]: an array of unknown bound declared once and completed by
// a later declaration (the out-of-class definition of a static data
// member; a namespace-scope definition after an `extern` declaration) is
// incomplete before that declaration and complete after it, for
// `decltype` of an id-expression or a member access and for `sizeof`.
// A later declaration that omits the bound keeps the earlier one.
// The reflection side (type_of) is meta.reflection.queries/
// type-of-completed-array.cpp. WP-795.
//@ expect: compile-ok
//@ strict-diags

template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;

namespace probe {
struct StaticProbe {
  static const char k_NAME[];
};
static_assert(same<decltype(StaticProbe::k_NAME), const char[]>);
inline const char StaticProbe::k_NAME[] = "SAFE";
} // namespace probe

static_assert(same<decltype(probe::StaticProbe::k_NAME), const char[5]>);
static_assert(sizeof(probe::StaticProbe::k_NAME) == 5);
static_assert(same<decltype(&probe::StaticProbe::k_NAME), const char (*)[5]>);
probe::StaticProbe object;
static_assert(same<decltype(object.k_NAME), const char[5]>);
static_assert(sizeof(object.k_NAME) == 5);

extern const int ns_array[];
static_assert(same<decltype(ns_array), const int[]>);
constexpr int ns_array[] = {1, 2, 3};
static_assert(same<decltype(ns_array), const int[3]>);
static_assert(sizeof(ns_array) == 3 * sizeof(int));

int bounded[3];
extern int bounded[];
static_assert(same<decltype(bounded), int[3]>);
static_assert(sizeof(bounded) == 3 * sizeof(int));

int main() {}
