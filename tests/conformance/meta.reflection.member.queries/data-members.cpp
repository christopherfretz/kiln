// [meta.reflection.member.queries]/5-12 (P2996R13): bases_of lists the
// direct base class relationships in declaration order;
// static_data_members_of and nonstatic_data_members_of the data members
// (an unnamed bit-field is not a non-static data member's reflection the
// query yields); subobjects_of the bases then the non-static data
// members. Each is filtered by the access context.
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;

constexpr auto all = access_context::unchecked();
constexpr auto pub = access_context::unprivileged();

struct B1 { char b1; };
struct B2 { short b2; };
struct V { int v[3]; };
struct D : B1, private B2, virtual V {
  int a;
  static int s;
  int : 3;
  int bf : 4;
  static const int t = 1;

 private:
  int p;
  static int ps;
};

static_assert(bases_of(^^D, all).size() == 3);
static_assert(bases_of(^^D, pub).size() == 2);
// (size_of tells the bases apart until type_of answers for them.)
static_assert(size_of(bases_of(^^D, all)[0]) == sizeof(B1));
static_assert(size_of(bases_of(^^D, all)[1]) == sizeof(B2));
static_assert(size_of(bases_of(^^D, all)[2]) == sizeof(V));
static_assert(size_of(bases_of(^^D, pub)[1]) == sizeof(V));

static_assert(static_data_members_of(^^D, all).size() == 3);
static_assert(static_data_members_of(^^D, all)[0] == ^^D::s);
static_assert(static_data_members_of(^^D, all)[1] == ^^D::t);
static_assert(identifier_of(static_data_members_of(^^D, all)[2]) == "ps");
static_assert(static_data_members_of(^^D, pub).size() == 2);

static_assert(nonstatic_data_members_of(^^D, all).size() == 3);
static_assert(nonstatic_data_members_of(^^D, all)[0] == ^^D::a);
static_assert(nonstatic_data_members_of(^^D, all)[1] == ^^D::bf);
static_assert(identifier_of(nonstatic_data_members_of(^^D, all)[2]) == "p");
static_assert(nonstatic_data_members_of(^^D, pub).size() == 2);

static_assert(subobjects_of(^^D, all).size() == 6);
static_assert(subobjects_of(^^D, all)[0] == bases_of(^^D, all)[0]);
static_assert(subobjects_of(^^D, all)[3] == ^^D::a);
static_assert(subobjects_of(^^D, pub).size() == 4);

// The same base-specifier reflection each time. (GCC 16.1 has an internal
// compiler error comparing two distinct ones, in cp_tree_equal.)
static_assert(bases_of(^^D, all)[1] == bases_of(^^D, all)[1]);
#ifdef __KILN__
static_assert(bases_of(^^D, all)[0] != bases_of(^^D, all)[1]);
#endif

// No bases, no data members.
struct E {};
static_assert(bases_of(^^E, all).size() == 0);
static_assert(nonstatic_data_members_of(^^E, all).size() == 0);
static_assert(subobjects_of(^^E, all).size() == 0);

// A class template specialization's bases are its own.
template <class T>
struct Tm : T { T m; };
static_assert(size_of(bases_of(^^Tm<B2>, all)[0]) == sizeof(B2));
static_assert(type_of(nonstatic_data_members_of(^^Tm<B2>, all)[0]) == ^^B2);

int main() {}
