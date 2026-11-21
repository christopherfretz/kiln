// [basic.link]/14-17 (WP-1855): the template heads that stay valid next to
// exposure-template-head.cpp. A head naming only entities that are not
// TU-local; a TU-local template (in an unnamed namespace, or `static`)
// whose head names TU-local entities; a friend declaration (/15.3); a
// TU-local constant read without an odr-use (/14.4) and an enumerator of a
// TU-local enumeration (not a TU-local entity, /16.1) in a head; a generic
// lambda in the body of a function that is not inline, or in a variable's
// initializer when its closure type is not the variable's type (/15.1,
// 15.2 ignore both); a template of the private module fragment (/18
// excludes it). GCC 16.1 rejects `reads_constant`, `enumerator` and
// `in_private`; clang 22 accepts all of it.
//@ expect: compile-ok
//@ std: c++20
//@ skip(gcc): GCC 16.1 rejects reads_constant, enumerator and in_private
//@ strict-diags
export module M;
static bool helper() { return true; }
static constexpr int K = 1;
namespace { template <class T> concept Hid = true; enum E { e0 }; struct S {}; }
template <class T> concept Pub = true;
export template <Pub T> void pub1(T);
export template <class T> requires Pub<T> && (sizeof(T) > 0) struct Pub2 {};
namespace { template <Hid T> void tu_local(T); template <class T> requires (helper()) struct TuLocal {}; }
template <Hid T> static void tu_local_static(T);
export struct Fr { template <Hid T> friend void fr(T); };
export template <class T> requires (K > 0) void reads_constant();
export template <int N = e0> void enumerator();
export void not_inline() { auto l = []<Hid T>(T) {}; l(1); }
export int lambda_called = []<Hid T>(T) { return 0; }(1);
export int lambda_param_called = [](S) { return 1; }(S());
module :private;
template <Hid T> void in_private(T);
