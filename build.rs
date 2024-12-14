fn main() {
    let config =
    slint_build::CompilerConfiguration::new()
    .with_style("cosmic".into());
    if let Ok(_compiler) = slint_build::compile_with_config("ui/app-window.slint", config){

    }
}
