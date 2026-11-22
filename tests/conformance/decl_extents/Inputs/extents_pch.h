// The prefix of pch.cpp, precompiled with the declaration-extent table on.
#define PCH_DECL(n) int n = 0;
namespace pch_ns {
PCH_DECL(from_pch)
template <class T> struct Box {
  T value;
  T get() const { return value; }
};
} // namespace pch_ns
extern "C" int pch_c(int);
