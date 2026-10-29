//@ expect: ast
//@ oracle: clang
// [expr.prim.req.compound]: compound requirements
template <class T, class U> concept Same = true;
template <class T> concept C = requires (T t) {
  { t.f() };
  { t.g() } noexcept;
  { t + 1 } -> Same<int>;
  { t.h() } noexcept -> Same<long>;
};
