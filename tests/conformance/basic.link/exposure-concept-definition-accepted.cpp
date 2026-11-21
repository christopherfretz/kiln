// [basic.link]/14-17 (WP-1845): the concepts that stay valid next to
// exposure-concept-definition.cpp. A TU-local constant read without an
// odr-use (/14.4); an enumerator of a TU-local enumeration (not a TU-local
// entity, /15.1); a concept that is itself TU-local (in an unnamed
// namespace); a concept of the global module fragment naming one of its
// static functions (not checked, as GCC 16.1 only warns there); a concept
// whose constraint holds a lambda naming nothing TU-local (CWG2988, open,
// would make its closure type TU-local; kiln does not treat a closure type
// as TU-local); a concept naming non-TU-local entities only; a concept in
// the private module fragment. GCC 16.1 rejects `reads_constant` and
// `enumerator` ("exposes TU-local entity"); clang 22 accepts it all.
//@ expect: compile-ok
//@ std: c++20
//@ skip(gcc): GCC 16.1 rejects reads_constant and enumerator
//@ strict-diags
module;
static int gmf_helper() { return 1; }
template <class T> concept gmf_concept = sizeof(gmf_helper()) > 0;
export module M;
static int helper() { return 1; }
static const int K = 4;
namespace { enum { e1 }; }
int visible();
export template <class T> concept reads_constant = K > 0;
export template <class T> concept enumerator = e1 == 0;
namespace { template <class T> concept tu_local = sizeof(helper()) > 0; }
export template <class T> concept lambda_only = [] { return true; }();
export template <class T> concept plain = requires(T t) { visible(); t + 1; };
export template <class T> concept uses_gmf = gmf_concept<T>;
export inline bool use() { return reads_constant<int> && plain<int> && lambda_only<int>; }
module :private;
template <class T> concept in_private = sizeof(helper()) > 0;
