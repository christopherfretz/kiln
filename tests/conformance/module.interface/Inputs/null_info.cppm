// Interface unit of module `ninfo` for null-info-run.cpp: default-
// initialized `std::meta::info` constants and a class holding one.
module;
export module ninfo;
export using info = decltype(^^int);
export constexpr info mnull;
export constexpr info marr[2];
export struct Holder {
  info m;
  int k = 3;
};
export constexpr Holder mh;
export template <info R> constexpr int pick = R == info{} ? 1 : 2;
