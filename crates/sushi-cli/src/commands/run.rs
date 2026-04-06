use clap::Args;

#[derive(Args)]
pub struct RunArgs {
    /// Plugin name to run
    pub plugin_name: String,

    /// Arguments to pass to the plugin
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,
}
