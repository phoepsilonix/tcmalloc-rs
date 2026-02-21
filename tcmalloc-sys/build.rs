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
        let autogen = gperftools_dir.join("autogen.sh");
        let mut autogen_cmd = Command::new("sh");
        autogen_cmd.arg(autogen).current_dir(&gperftools_dir);
        run(&mut autogen_cmd);

        let configure = gperftools_dir.join("configure");
        let mut configure_cmd = Command::new("sh");
        configure_cmd.arg(configure).current_dir(&build_dir);
        // autotools 的には
        //   --build = コンパイルしているマシン (Rust の HOST)
        //   --host  = 生成されるバイナリが動くマシン (Rust の TARGET)
        configure_cmd.arg(format!("--build={host}"));
        configure_cmd.arg(format!("--host={target}"));

        // ★ muslターゲット用の重要設定
        if target.contains("musl") {
            configure_cmd.arg("--disable-shared");  // staticのみ
            configure_cmd.arg("--enable-static");   // static有効化
            configure_cmd.arg("--disable-frame-pointers");  // 不要な依存削減
            //configure_cmd.arg("--disable-unwind");
            configure_cmd.arg("--disable-libunwind");          // ← これを追加（libunwind 無効）
            configure_cmd.arg("--with-tcmalloc-pagesize=4096"); // 任意だが musl で安定しやすい
            configure_cmd.arg("--enable-minimal");             // 最小構成（tcmalloc_minimal だけビルド）
            // またはもっと厳密に
            configure_cmd.arg("--disable-heap-profiler");
            configure_cmd.arg("--disable-heap-checker");
            configure_cmd.arg("--disable-profiler");           // これで unwind 依存がかなり減る

            // CFLAGS で強制的にフレームポインタ無効（古い gperftools で unwind 回避）
            configure_cmd.arg("CFLAGS=-fno-omit-frame-pointer -fno-stack-protector -D_GNU_SOURCE -DBENCHMARK_OS_LINUX");    
            // libstdc++を明示的にリンク（musl環境でも必要）
            //configure_cmd.env("CXX", "zig c++");
            //configure_cmd.env("CC", "zig cc");
            //configure_cmd.env("LD", "zig c++");  // linkerもZig

            // ABI問題回避
            //configure_cmd.env("CXXFLAGS", "-D_GLIBCXX_USE_CXX11_ABI=1");
            // prefix付きでcross tools警告解消
            //configure_cmd.env("PATH", format!("{}:{}", env::var("PATH").unwrap(), "/usr/bin"));
        }
        run(&mut configure_cmd);
    }

    let mut make_cmd = Command::new("make");
    make_cmd
        .current_dir(&build_dir)
        .arg("srcroot=../gperftools/")
        .arg("-j")
        .arg(num_jobs);
    run(&mut make_cmd);

    println!("cargo:rustc-link-lib=static=tcmalloc");
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
