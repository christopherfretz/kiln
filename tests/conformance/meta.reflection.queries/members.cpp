// [meta.reflection.queries] P2996R13, [class.mem], [class.access]: the
// member predicates (is_class_member, is_namespace_member,
// is_nonstatic_data_member, is_static_member, is_bit_field,
// is_mutable_member, has_default_member_initializer, is_base) and the
// access predicates (is_public, is_protected, is_private). A member of an
// anonymous union is a member of the union, a direct member of the class
// for lookup only; an enumerator of an unscoped enumeration declared in a
// class is a class member, one of a scoped enumeration is not. A function
// template is a static member when its pattern is a static member
// function. An entity that is not a member has no access, so is_public is
// false for it. Answers checked against GCC 16.1 (-freflection) except
// one: GCC 16.1 answers is_namespace_member(^^int) == true for a
// fundamental type, which is not a member of any namespace (kiln follows
// the IS). is_base is true only for a direct base relationship, as
// bases_of returns; the relationship has the access of its
// base-specifier.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta_queries.h"
using namespace std::meta;

namespace N { int nv; struct NS {}; }
using A = int;
template <class T> struct CT {};
int gv;
enum UE { u1 };
enum class EC { a };
struct S {
  int m;
  static int sm;
  mutable int mu;
  int bf : 3;
  int dmi = 4;
  const int cm = 1;
  void f();
  static void sf();
  template <class T> static void stf();
  template <class T> void tf();
  using MA = int;
  struct In {};
  enum E { e1 };
  enum class SE { se };
  union { int au; long al; };
private:
  int pm;
  struct PIn {};
protected:
  int prm;

  static_assert(is_private(^^pm) && is_private(^^PIn) && is_protected(^^prm));
  static_assert(!is_public(^^pm) && !is_protected(^^pm) && !is_private(^^prm));
};

static_assert(is_class_member(^^S::m) && is_class_member(^^S::sm) && is_class_member(^^S::f));
static_assert(is_class_member(^^S::MA) && is_class_member(^^S::In) && is_class_member(^^S::E));
static_assert(is_class_member(^^S::e1) && !is_class_member(^^S::SE::se));
static_assert(is_class_member(^^S::stf) && is_class_member(^^S::au));
static_assert(!is_class_member(^^S) && !is_class_member(^^gv) && !is_class_member(^^int));
static_assert(!is_class_member(^^EC::a) && !is_namespace_member(^^EC::a));

static_assert(is_namespace_member(^^N::nv) && is_namespace_member(^^N::NS) && is_namespace_member(^^N));
static_assert(is_namespace_member(^^A) && is_namespace_member(^^CT) && is_namespace_member(^^S));
static_assert(is_namespace_member(^^gv) && is_namespace_member(^^u1) && is_namespace_member(^^UE));
static_assert(!is_namespace_member(^^::) && !is_namespace_member(^^S::m));
static_assert(!is_namespace_member(^^int));

static_assert(is_nonstatic_data_member(^^S::m) && is_nonstatic_data_member(^^S::bf));
static_assert(is_nonstatic_data_member(^^S::au) && is_nonstatic_data_member(^^S::cm));
static_assert(!is_nonstatic_data_member(^^S::sm) && !is_nonstatic_data_member(^^S::f));
static_assert(!is_nonstatic_data_member(^^gv));

static_assert(is_static_member(^^S::sm) && is_static_member(^^S::sf) && is_static_member(^^S::stf));
static_assert(!is_static_member(^^S::m) && !is_static_member(^^S::f) && !is_static_member(^^S::tf));
static_assert(!is_static_member(^^gv) && !is_static_member(^^S::In));

static_assert(is_mutable_member(^^S::mu) && !is_mutable_member(^^S::m) && !is_mutable_member(^^S::sm));
static_assert(is_bit_field(^^S::bf) && !is_bit_field(^^S::m) && !is_bit_field(^^gv));
static_assert(has_default_member_initializer(^^S::dmi) && has_default_member_initializer(^^S::cm));
static_assert(!has_default_member_initializer(^^S::m) && !has_default_member_initializer(^^S::sm));

static_assert(is_public(^^S::m) && is_public(^^S::In) && is_public(^^S::e1) && is_public(^^S::au));
static_assert(!is_protected(^^S::m) && !is_private(^^S::m));
static_assert(!is_public(^^gv) && !is_public(^^S) && !is_public(^^int) && !is_private(^^::));
static_assert(!is_public(^^S::SE::se));

static_assert(!is_base(^^S) && !is_base(^^S::m) && !is_base(info{}));
struct B1 {};
struct B2 {};
struct D : B1, private B2 {};
constexpr info db0 = bases_of(^^D, access_context::unchecked())[0];
constexpr info db1 = bases_of(^^D, access_context::unchecked())[1];
static_assert(is_base(db0) && is_base(db1));
static_assert(is_public(db0) && !is_private(db0) && is_private(db1) && !is_public(db1));
static_assert(!is_class_member(db0) && !is_namespace_member(db0));
