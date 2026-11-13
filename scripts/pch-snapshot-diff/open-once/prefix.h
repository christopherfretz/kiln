// The header itself is #pragma once: the main file's #include of it
// again is skipped.
#pragma once
inline int pfx_once() { return 4; }
struct PfxOnce { int v = pfx_once(); };
