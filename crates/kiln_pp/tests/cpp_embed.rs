//! [cpp.embed] Resource inclusion through the `IncludeResolver` (here `MemoryResolver`).
//!
//! Goldens: `clang++-19 -std=c++26 -E -P` (generated).
#![allow(clippy::needless_raw_string_hashes, clippy::doc_markdown, reason = "generated goldens")]

mod common;

use kiln_pp::{MemoryResolver, PrintOptions};

/// [cpp.embed.gen]/1
#[test]
fn embed_basic() {
    let r = MemoryResolver::new().with_resource("d.bin", b"abc");
    common::expect(
        &common::run_with(
            r#"int a[] = {
#embed "d.bin"
};
"#,
            r,
            PrintOptions::default(),
            |_| {},
        ),
        r#"int a[] = {97, 98, 99 };
"#,
        &[r#"2:2: warning: #embed is a Clang extension"#],
    );
}

/// [cpp.embed.gen]/1
#[test]
fn embed_angled() {
    let r = MemoryResolver::new().with_resource("d.bin", b"abc");
    common::expect(
        &common::run_with(
            r#"int a[] = {
#embed <d.bin>
};
"#,
            r,
            PrintOptions::default(),
            |_| {},
        ),
        r#"int a[] = {97, 98, 99 };
"#,
        &[r#"2:2: warning: #embed is a Clang extension"#],
    );
}

/// [cpp.embed.gen]/1
#[test]
fn embed_binary_bytes() {
    let r = MemoryResolver::new().with_resource("d.bin", b"\x00\xff\x0a\x7f");
    common::expect(
        &common::run_with(
            r#"int a[] = {
#embed "d.bin"
};
"#,
            r,
            PrintOptions::default(),
            |_| {},
        ),
        r#"int a[] = {0, 255, 10, 127 };
"#,
        &[r#"2:2: warning: #embed is a Clang extension"#],
    );
}

/// [cpp.embed.param.limit]/1
#[test]
fn embed_limit() {
    let r = MemoryResolver::new().with_resource("d.bin", b"abc");
    common::expect(
        &common::run_with(
            r#"int a[] = {
#embed "d.bin" limit(2)
};
"#,
            r,
            PrintOptions::default(),
            |_| {},
        ),
        r#"int a[] = {97, 98 };
"#,
        &[r#"2:2: warning: #embed is a Clang extension"#],
    );
}

/// [cpp.embed.param.if.empty]/1
#[test]
fn embed_limit_zero_if_empty() {
    let r = MemoryResolver::new().with_resource("d.bin", b"abc");
    common::expect(
        &common::run_with(
            r#"int a[] = {
#embed "d.bin" limit(0) if_empty(7)
};
"#,
            r,
            PrintOptions::default(),
            |_| {},
        ),
        r#"int a[] = {7};
"#,
        &[r#"2:2: warning: #embed is a Clang extension"#],
    );
}

/// [cpp.embed.param.limit]/1
#[test]
fn embed_limit_macro() {
    let r = MemoryResolver::new().with_resource("d.bin", b"abc");
    common::expect(
        &common::run_with(
            r#"#define N 1
int a[] = {
#embed "d.bin" limit(N)
};
"#,
            r,
            PrintOptions::default(),
            |_| {},
        ),
        r#"
int a[] = {97 };
"#,
        &[r#"3:2: warning: #embed is a Clang extension"#],
    );
}

/// [cpp.embed.param.prefix]/1
/// [cpp.embed.param.suffix]/1
#[test]
fn embed_prefix_suffix() {
    let r = MemoryResolver::new().with_resource("d.bin", b"abc");
    common::expect(
        &common::run_with(
            r#"int a[] = {
#embed "d.bin" prefix(0, ) suffix(, 9)
};
"#,
            r,
            PrintOptions::default(),
            |_| {},
        ),
        r#"int a[] = {0,97, 98, 99 , 9};
"#,
        &[r#"2:2: warning: #embed is a Clang extension"#],
    );
}

/// [cpp.embed.param.prefix]/1
/// [cpp.embed.param.if.empty]/1
#[test]
fn embed_empty_prefix_suffix_dropped() {
    let r = MemoryResolver::new().with_resource("e.bin", b"");
    common::expect(
        &common::run_with(
            r#"int a[] = {
#embed "e.bin" prefix(1, ) suffix(, 2) if_empty(-1)
};
"#,
            r,
            PrintOptions::default(),
            |_| {},
        ),
        r#"int a[] = {-1};
"#,
        &[r#"2:2: warning: #embed is a Clang extension"#],
    );
}

/// [cpp.embed.param.if.empty]/1
#[test]
fn embed_empty_if_empty() {
    let r = MemoryResolver::new().with_resource("e.bin", b"");
    common::expect(
        &common::run_with(
            r#"int a[] = {
#embed "e.bin" if_empty(42)
};
"#,
            r,
            PrintOptions::default(),
            |_| {},
        ),
        r#"int a[] = {42};
"#,
        &[r#"2:2: warning: #embed is a Clang extension"#],
    );
}

/// [cpp.embed.gen]/1
#[test]
fn embed_empty_nothing() {
    let r = MemoryResolver::new().with_resource("e.bin", b"");
    common::expect(
        &common::run_with(
            r#"x
#embed "e.bin"
y
"#,
            r,
            PrintOptions::default(),
            |_| {},
        ),
        r#"x
y
"#,
        &[r#"2:2: warning: #embed is a Clang extension"#],
    );
}

/// [cpp.embed.param]/1
#[test]
fn embed_clang_prefixed_params() {
    let r = MemoryResolver::new().with_resource("d.bin", b"abc");
    common::expect(
        &common::run_with(
            r#"int a[] = {
#embed "d.bin" __limit__(1) clang::offset(1)
};
"#,
            r,
            PrintOptions::default(),
            |_| {},
        ),
        r#"int a[] = {98 };
"#,
        &[r#"2:2: warning: #embed is a Clang extension"#],
    );
}

/// [cpp.embed.gen]/1
#[test]
fn embed_not_found() {
    common::check_diags(
        r#"#embed "nope.bin"
after
"#,
        r#"
after
"#,
        &[
            r#"1:2: warning: #embed is a Clang extension"#,
            r#"1:8: fatal error: 'nope.bin' file not found"#,
        ],
    );
}

/// [cpp.embed.param]/1
#[test]
fn embed_unknown_param() {
    let r = MemoryResolver::new().with_resource("d.bin", b"abc");
    common::expect(
        &common::run_with(
            r#"#embed "d.bin" bogus(1)
"#,
            r,
            PrintOptions::default(),
            |_| {},
        ),
        r#"
"#,
        &[
            r#"1:2: warning: #embed is a Clang extension"#,
            r#"1:24: error: unknown embed preprocessor parameter 'bogus'"#,
        ],
    );
}

/// [cpp.embed.param]/1
#[test]
fn embed_duplicate_param() {
    let r = MemoryResolver::new().with_resource("d.bin", b"abc");
    common::expect(
        &common::run_with(
            r#"#embed "d.bin" limit(1) limit(2)
"#,
            r,
            PrintOptions::default(),
            |_| {},
        ),
        r#"97, 98
"#,
        &[
            r#"1:2: warning: #embed is a Clang extension"#,
            r#"1:30: error: cannot specify parameter 'limit' twice in the same '#embed' directive"#,
        ],
    );
}

/// [cpp.embed.param.limit]/1
#[test]
fn embed_negative_limit() {
    let r = MemoryResolver::new().with_resource("d.bin", b"abc");
    common::expect(
        &common::run_with(
            r#"#embed "d.bin" limit(-1)
"#,
            r,
            PrintOptions::default(),
            |_| {},
        ),
        r#"
"#,
        &[
            r#"1:2: warning: #embed is a Clang extension"#,
            r#"1:25: error: invalid value '-1'; must be positive"#,
        ],
    );
}

/// [cpp.embed.gen]/1
#[test]
fn embed_macro_name() {
    let r = MemoryResolver::new().with_resource("d.bin", b"abc");
    common::expect(
        &common::run_with(
            r#"#define F "d.bin"
int a[] = {
#embed F
};
"#,
            r,
            PrintOptions::default(),
            |_| {},
        ),
        r#"
int a[] = {97, 98, 99 };
"#,
        &[r#"3:2: warning: #embed is a Clang extension"#],
    );
}

/// [cpp.cond]/5
#[test]
fn has_embed_values() {
    let r = MemoryResolver::new().with_resource("d.bin", b"abc").with_resource("e.bin", b"");
    common::expect(
        &common::run_with(
            r#"#if __has_embed("d.bin") == 1
found
#endif
#if __has_embed("e.bin") == 2
empty
#endif
#if __has_embed("nope.bin") == 0
missing
#endif
#if __has_embed("d.bin" limit(0)) == 2
limit0
#endif
#if __has_embed("d.bin" bogus) == 0
unsupported
#endif
"#,
            r,
            PrintOptions::default(),
            |_| {},
        ),
        r#"
found
empty
missing
limit0
unsupported
"#,
        &[],
    );
}

/// [cpp.cond]/5
#[test]
fn has_embed_defined() {
    common::check(
        r#"#ifdef __has_embed
yes
#endif
"#,
        r#"
yes
"#,
    );
}
