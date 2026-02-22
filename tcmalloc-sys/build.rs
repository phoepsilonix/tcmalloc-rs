use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

static TCMALLOC_REPO: &str = "https://github.com/gperftools/gperftools";
static TCMALLOC_TAG: &str = "gperftools-2.18";

// Platforms that _someone_ says works
static TESTED: &[&str] = &[
    "aarch64-apple-darwin",
    "aarch64-unknown-linux-gnu",
    "aarch64-unknown-linux-musl",
    "aarch64-pc-windows-msvc",
    "armv7-unknown-linux-gnueabihf",
    "armv7-unknown-linux-musleabihf",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "x86_64-pc-windows-msvc",
];

fn main() {
    let target = env::var("TARGET").expect("TARGET was not set");
    let host = env::var("HOST").expect("HOST was not set");
    let num_jobs = env::var("NUM_JOBS").expect("NUM_JOBS was not set");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR was not set"));
    let src_dir = env::current_dir().expect("failed to get current directory");
    let build_dir = out_dir.join("build");
    let gperftools_dir = out_dir.join("gperftools");

    println!("TARGET={}", target.clone());
    println!("HOST={}", host.clone());
    println!("NUM_JOBS={}", num_jobs.clone());
    println!("OUT_DIR={:?}", out_dir);
    println!("BUILD_DIR={:?}", build_dir);
    println!("SRC_DIR={:?}", src_dir);
    println!("GPERFTOOLS_DIR={:?}", gperftools_dir);

    if !TESTED.contains(&target.as_ref()) {
        println!(
            "cargo:warning=tcmalloc-rs has not been verified to work on target {}",
            target
        );
        //return;
    }

    // Clone source to OUT_DIR
    if !out_dir.join("gperftools").exists() {
        assert!(out_dir.exists(), "OUT_DIR does not exist");
        let mut cmd = Command::new("git");
        cmd.current_dir(&out_dir).args(&[
            "clone",
            TCMALLOC_REPO,
            "--depth=1",
            "--branch",
            TCMALLOC_TAG,
        ]);
        run(&mut cmd);
    }

    fs::create_dir_all(&build_dir).unwrap();

    // Only run configure once
    if !build_dir.join("Makefile").exists() {
        // autogen.sh
        let autogen = gperftools_dir.join("autogen.sh");
        let mut autogen_cmd = Command::new("sh");
        autogen_cmd.arg(autogen).current_dir(&gperftools_dir);
        run(&mut autogen_cmd);

        // configure
        let mut configure_cmd = Command::new("sh");
        configure_cmd
            .arg(gperftools_dir.join("configure"))
            .current_dir(&build_dir)
            .arg("--verbose")
            .arg(format!("--build={}", host))
            .arg(format!("--host={}", target))
            .arg("--disable-shared")
            .arg("--enable-static");

        if target.contains("musl") {
            //configure_cmd.arg("--enable-minimal");

            // disabled unwind, stacktrace
            //.arg("--disable-backtrace")
            configure_cmd
                .arg("--disable-libunwind")
                .arg("--enable-cxx-stdlib")
                .arg("--disable-stacktrace-via-backtrace")
                .arg("--enable-frame-pointers")
                .arg("--disable-cpu-profiler")
                .arg("--disable-heap-profiler")
                .arg("--disable-heap-checker")
                .arg("--disable-debugalloc");

            //"-D_GNU_SOURCE {} -D__GLIBC_USE_C2X_STRTOL=0 -U__GLIBC_PREREQ -D__GLIBC__=2 -D__GLIBC_MINOR__=27 -U__STRICT_ANSI__ -D_FORTIFY_SOURCE=0 -U_FORTIFY_SOURCE",
            let cflags = format!(
                "-D_GNU_SOURCE {} -D__GLIBC__=2 -D__GLIBC_MINOR__=27 -D_FORTIFY_SOURCE=0 -U_FORTIFY_SOURCE",
                env::var("CFLAGS").unwrap_or_default()
            );
            let cxxflags = format!(
                "-D_GNU_SOURCE {} -D__GLIBC__=2 -D__GLIBC_MINOR__=27 -D_FORTIFY_SOURCE=0 -U_FORTIFY_SOURCE {}",
                env::var("CXXFLAGS").unwrap_or_default(),
                "-std=c++17");
            configure_cmd
                .env("CFLAGS", cflags)
                .env("CXXFLAGS", cxxflags)
                .env("PKG_CONFIG_ALLOW_CROSS", "1") // pkg-config が cross 時にも動くように（稀に必要）
                .env("FORCE_UNSAFE_CONFIGURE", "1"); // 古い autotools で cross 警告を無視したい場合
        }
        run(&mut configure_cmd);
    }

    let mut make_cmd = Command::new("make");
    make_cmd
        .current_dir(&build_dir)
        .arg("srcroot=../gperftools/")
        .arg("V=1")
        .arg("libtcmalloc.la")
        .arg("libtcmalloc_minimal.la")
        .arg("-j")
        .arg(num_jobs);
    run(&mut make_cmd);

    // static link
    println!("cargo:rustc-link-lib=static=tcmalloc_minimal");

    println!(
        "cargo:rustc-link-search=native={}/.libs",
        build_dir.display()
    );
    println!("cargo:rerun-if-changed=gperftools");
}

fn run(cmd: &mut Command) {
    println!("running: {:?}", cmd);
    let status = match cmd.status() {
        Ok(status) => status,
        Err(e) => panic!("failed to execute command: {}", e),
    };
    if !status.success() {
        panic!(
            "command did not execute successfully: {:?}\n\
             expected success, got: {}",
            cmd, status
        );
    }
}
