#pragma once

inline int pch_used_ran = 0;
inline int pch_ctor_ran = 0;

inline void pch_used() {
  struct L {
    [[gnu::used]] static void c() { ++pch_used_ran; }
  };
}

inline void pch_ctor() {
  struct L {
    [[gnu::constructor]] static void c() { ++pch_ctor_ran; }
  };
}
