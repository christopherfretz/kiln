// [class.copy.elision]/1.3 (C++14): an elidable copy from a prvalue with
// a usable copy or move constructor; in unevaluated operands none is
// needed.
//@ std: c++14
struct Mv { Mv(); Mv(Mv&&) = default; };
Mv mk();
Mv a() { Mv m = mk(); return mk(); }
struct Im { Im(Im&&) = delete; };
Im imk();
static_assert(sizeof(imk()) == 1, "");
decltype(imk()) *p = nullptr;
struct W { W() : n(mk()) {} Mv n; };
