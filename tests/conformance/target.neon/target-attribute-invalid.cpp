// [dcl.attr] An unknown `cpu=` or `tune=` CPU (case-sensitive), a
// repeated `arch=`/`cpu=`/`tune=`, or an `fpmath=` in a `target("...")`
// attribute is a -Wignored-attributes warning and the attribute is
// ignored; an unknown `arch=` level (or a case mismatch) is silently a
// no-op. clang `Sema::checkTargetAttr` (spec/driver.md "Target
// features", target attribute). `verify` checks the warnings
// exhaustively. WP-1187.
//@ requires: target-aarch64
//@ verify
__attribute__((target("cpu=foo"))) void b2() {} // expected-warning {{unknown CPU 'foo' in the 'target' attribute string; 'target' attribute ignored}}
__attribute__((target("tune=bar"))) void b3() {} // expected-warning {{unknown tune CPU 'bar' in the 'target' attribute string; 'target' attribute ignored}}
__attribute__((target("arch=armv8.1-a,arch=armv8.2-a"))) void b6() {} // expected-warning {{duplicate 'arch=' in the 'target' attribute string; 'target' attribute ignored}}
__attribute__((target("cpu=Neoverse-V1"))) void b7() {} // expected-warning {{unknown CPU 'Neoverse-V1' in the 'target' attribute string; 'target' attribute ignored}}
__attribute__((target("cpu=cortex-a57,cpu=neoverse-v1"))) void b9() {} // expected-warning {{duplicate 'cpu=' in the 'target' attribute string; 'target' attribute ignored}}
__attribute__((target("tune=cortex-a57,tune=neoverse-v1"))) void b10() {} // expected-warning {{duplicate 'tune=' in the 'target' attribute string; 'target' attribute ignored}}
__attribute__((target("fpmath=sse"))) void b11() {} // expected-warning {{unsupported 'fpmath=' in the 'target' attribute string; 'target' attribute ignored}}
__attribute__((target("cpu=native"))) void b17() {} // expected-warning {{unknown CPU 'native' in the 'target' attribute string; 'target' attribute ignored}}
__attribute__((target("cpu=foo,tune=bar,arch=armv8-a,arch=armv8-a"))) void b18() {} // expected-warning {{unknown CPU 'foo' in the 'target' attribute string; 'target' attribute ignored}}
__attribute__((target("arch=armv7-a"))) void b1() {}
__attribute__((target("arch=ARMv8.2-a"))) void b8() {}
