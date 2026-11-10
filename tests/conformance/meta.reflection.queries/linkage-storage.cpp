// [meta.reflection.queries] P2996R13, [basic.link], [basic.stc]: the
// linkage predicates (has_internal_linkage, has_module_linkage,
// has_external_linkage, has_c_language_linkage, has_linkage) and the
// storage-duration predicates (has_static_storage_duration,
// has_thread_storage_duration, has_automatic_storage_duration). Linkage
// belongs to a name ([basic.link]/2), so a type alias, a non-static data
// member, an enumerator of a scoped enumeration, a fundamental type, a
// closure type (at namespace or class scope too) and a local entity have
// none; a reflection of a type answers for the class or
// enumeration's own name. Answers checked against GCC 16.1
// (-freflection) except one: GCC 16.1 answers has_linkage(^^NA) == true
// for a namespace alias, which [basic.link] does not give linkage (kiln
// follows the IS).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ xfail(gcc): GCC 16.1's has_linkage disagrees on entities without linkage (a namespace alias among them)
#include "kiln_meta_queries.h"
using namespace std::meta;

using A = int;
namespace N { int nv; struct NS {}; }
namespace NA = N;
namespace { int anon_v; struct AnonS {}; }
static int sv;
const int cv = 1;
extern const int ecv;
inline const int icv = 2;
int ev;
thread_local int tv;
struct S {
  int m;
  static int sm;
  void f();
  struct In {};
  using MA = int;
};
enum class EC { a };
template <class T> struct CT {};
extern "C" void cfun();
extern "C" int cvar;
extern "C" { int cvar2; }
void g(int);
auto lam = [] {};
inline auto ilam = [] {};
struct SL { static constexpr auto m = [] {}; };

static_assert(has_external_linkage(^^N) && has_external_linkage(^^N::nv));
static_assert(has_external_linkage(^^N::NS) && has_linkage(^^N::NS));
static_assert(!has_linkage(^^NA));
static_assert(has_internal_linkage(^^anon_v) && has_internal_linkage(^^AnonS));
static_assert(!has_external_linkage(^^anon_v) && has_linkage(^^anon_v));
static_assert(has_internal_linkage(^^sv));
static_assert(has_internal_linkage(^^cv));  // [basic.link]/3.2: const, not inline, not extern
static_assert(has_external_linkage(^^ecv) && has_external_linkage(^^icv));
static_assert(has_external_linkage(^^ev) && has_external_linkage(^^g));
static_assert(has_external_linkage(^^S) && has_external_linkage(^^S::sm));
static_assert(has_external_linkage(^^S::f) && has_external_linkage(^^S::In));
static_assert(!has_linkage(^^S::m) && !has_linkage(^^S::MA) && !has_linkage(^^A));
static_assert(has_external_linkage(^^EC) && !has_linkage(^^EC::a));
static_assert(has_external_linkage(^^CT));
static_assert(!has_linkage(^^int) && !has_linkage(^^S*) && !has_linkage(info{}));
static_assert(!has_linkage(^^decltype(lam)) && !has_linkage(^^decltype(ilam)));
static_assert(!has_linkage(^^decltype(SL::m)));
static_assert(!has_module_linkage(^^ev) && !has_module_linkage(^^S));

static_assert(has_c_language_linkage(^^cfun) && has_c_language_linkage(^^cvar));
static_assert(has_c_language_linkage(^^cvar2));
static_assert(!has_c_language_linkage(^^g) && !has_c_language_linkage(^^S));

static_assert(has_static_storage_duration(^^ev) && has_static_storage_duration(^^sv));
static_assert(has_static_storage_duration(^^cv) && has_static_storage_duration(^^S::sm));
static_assert(!has_static_storage_duration(^^S::m) && !has_static_storage_duration(^^g));
static_assert(has_thread_storage_duration(^^tv) && !has_static_storage_duration(^^tv));
static_assert(!has_thread_storage_duration(^^ev) && !has_automatic_storage_duration(^^ev));

void h() {
  int loc = 0;
  static int sloc;
  thread_local int tloc;
  struct L {};
  static_assert(has_automatic_storage_duration(^^loc) && !has_linkage(^^loc));
  static_assert(has_static_storage_duration(^^sloc) && !has_linkage(^^sloc));
  static_assert(has_thread_storage_duration(^^tloc));
  static_assert(!has_linkage(^^L));
}
