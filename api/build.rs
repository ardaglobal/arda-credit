fn main() {
    // Ensure templates are included in the build
    println!("cargo:rerun-if-changed=templates/");
}
