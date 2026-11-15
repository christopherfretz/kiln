// Header of pch-null-info-run.cpp: default-initialized `std::meta::info`
// constants a precompiled header carries.
using info = decltype(^^int);
constexpr info pch_null;
constexpr info pch_null_arr[2];
struct PchInfo {
  info m;
  int k = 7;
};
constexpr PchInfo pch_holder;
