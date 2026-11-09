// C++14 [except.spec]/5, /14-15 (C++11/14): valid twin of
// exception-spec-implicit-dynamic.cpp. An implicit member's computed list
// is allowed when each of its types is caught by a handler of the base's
// list, and a callee that throws nothing adds nothing (WP-957).
//@ std: c++11
struct A {};
struct C : A {};
struct X0 { virtual ~X0() throw(A); };
struct MC { ~MC() throw(C); };
struct Y0 : X0 { MC m; };
struct MN { ~MN(); };
struct Y2 : X0 { MN m; };
struct Y4 : X0 {};
struct D;
struct BA { virtual D &operator=(const D &) throw(A); };
struct MA { MA &operator=(const MA &) throw(C); };
struct D : BA { MA m; };
