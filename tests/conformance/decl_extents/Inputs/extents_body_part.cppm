// The interface partition of module-bodies-image.cpp and
// module-bodies-replay.cpp: an exported module declaration with a
// partition, and declarations inside the bodies of its definitions.
export module extents_body:part;
export struct Pair { int a, b; };
export template <class T> concept Gettable = requires(T t, int n) { t.get(n); };
export inline int split(int a) {
  auto [x, y] = Pair{a, 2};
  return x + y;
}
