fn main() {
    // option_env! 은 카고가 기본으로 추적하지 않아, 태그만 바뀌면 캐시된
    // 바이너리가 그대로 쓰인다. 그걸 막는다
    println!("cargo::rerun-if-env-changed=GIT_SHA");
}
