#!/usr/bin/env python3
"""Synthesizes a header tree shaped like modular Boost, for measuring
header-search cost (WP-820, the WP-824 case of 157 `-I` directories).

    python3 scripts/perf/gen-include-tree.py <root> [N=157]

writes <root>/libs/<lib>/include/boost/... for N libraries plus
libs/config, a <root>/main.cpp that includes the last 20 libraries, and
<root>/flags.txt with one -I per library. Each library has 12 headers
(alternately include-guarded and `#pragma once`), each including a
sibling detail header with "...", <boost/config.hpp>, and headers of up
to 4 earlier libraries with <...>. Deterministic (seeded). Measurements:
docs/status/header-search-syscalls.md.
"""
import os, random, sys
root = sys.argv[1]; N = int(sys.argv[2]) if len(sys.argv) > 2 else 157
H = 12
random.seed(7)
libs = [f"lib{i:03d}" for i in range(N)]
for li, lib in enumerate(libs):
    inc = os.path.join(root, "libs", lib, "include", "boost")
    os.makedirs(os.path.join(inc, lib, "detail"), exist_ok=True)
    deps = random.sample(libs[:li], min(li, 4)) if li else []
    with open(os.path.join(inc, lib + ".hpp"), "w") as f:
        g = f"BOOST_{lib.upper()}_HPP"
        f.write(f"#ifndef {g}\n#define {g}\n")
        for h in range(H):
            f.write(f"#include <boost/{lib}/h{h}.hpp>\n")
        f.write("#endif\n")
    for h in range(H):
        with open(os.path.join(inc, lib, f"h{h}.hpp"), "w") as f:
            f.write("#pragma once\n" if h % 2 else f"#ifndef G_{lib}_{h}\n#define G_{lib}_{h}\n")
            f.write(f'#include "detail/d{h}.hpp"\n#include <boost/config.hpp>\n')
            for d in deps:
                f.write(f"#include <boost/{d}/h{random.randrange(H)}.hpp>\n#include <boost/{d}.hpp>\n")
            f.write(f"int {lib}_h{h};\n")
            if h % 2 == 0: f.write("#endif\n")
        with open(os.path.join(inc, lib, "detail", f"d{h}.hpp"), "w") as f:
            f.write(f"#pragma once\nint {lib}_d{h};\n")
cfg = os.path.join(root, "libs", "config", "include", "boost")
os.makedirs(cfg, exist_ok=True)
open(os.path.join(cfg, "config.hpp"), "w").write("#pragma once\n#define BOOST_CONFIG 1\n")
with open(os.path.join(root, "main.cpp"), "w") as f:
    for lib in libs[-20:]:
        f.write(f"#include <boost/{lib}.hpp>\n")
    f.write("int main() { return 0; }\n")
with open(os.path.join(root, "flags.txt"), "w") as f:
    f.write(" ".join(f"-I{root}/libs/{l}/include" for l in libs + ["config"]))
