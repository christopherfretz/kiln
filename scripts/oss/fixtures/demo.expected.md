### demo (v1.0.0)

| | kiln | clang-19 |
|---|---|---|
| configure | ok | ok |
| build | rc 2 | ok |
| TUs ok / failed | 1 / 4 | 4 / 1 |
| links ok / failed | 0 / 0 | 1 / 0 |
| tests passed / total | - | 4 / 4 |
| wall | 12 s | 9 s |
| peak TU RSS | 293 MB | 195 MB |

Failure clusters (kiln): 2.

1. **3x** `reject: error: no matching function for call to Q`; 1 also fails with clang
   - e.g. `/work/kiln/.oss/src/demo/test/a.cc`: `/work/kiln/.oss/src/demo/test/a.cc:12:3: error: no matching function for call to 'frob'`
   - repro: `cd /work/kiln/.oss/build/demo/kiln/test && kiln++ -std=gnu++20 -O3 -DNDEBUG -I/work/kiln/.oss/src/demo/include -o CMakeFiles/demo_test.dir/a.cc.o -c /work/kiln/.oss/src/demo/test/a.cc`
2. **1x** `panic: thread Q panicked at expr.rs:N:N: | not yet implemented`
   - e.g. `/work/kiln/.oss/src/demo/test/d.cc`: `thread 'main' panicked at crates/kiln_sema/src/expr.rs:812:17: | not yet implemented`
   - repro: `cd /work/kiln/.oss/build/demo/kiln/test && kiln++ -std=gnu++20 -O3 -DNDEBUG -I/work/kiln/.oss/src/demo/include -o CMakeFiles/demo_test.dir/d.cc.o -c /work/kiln/.oss/src/demo/test/d.cc`
