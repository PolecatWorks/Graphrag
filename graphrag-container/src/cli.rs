use clap::Parser;

#[derive(Parser, Debug, PartialEq)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Sets a custom config file
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_default_no_args() {
        let cli = Cli::try_parse_from(["app"]).unwrap();
        assert_eq!(cli, Cli { config: None });
    }

    #[test]
    fn test_cli_short_config_flag() {
        let cli = Cli::try_parse_from(["app", "-c", "config.yaml"]).unwrap();
        assert_eq!(
            cli,
            Cli {
                config: Some("config.yaml".to_string())
            }
        );
    }

    #[test]
    fn test_cli_long_config_flag() {
        let cli = Cli::try_parse_from(["app", "--config", "custom.yaml"]).unwrap();
        assert_eq!(
            cli,
            Cli {
                config: Some("custom.yaml".to_string())
            }
        );
    }

    #[test]
    fn test_cli_unknown_flag_fails() {
        let result = Cli::try_parse_from(["app", "--unknown-flag"]);
        assert!(result.is_err());
    }
}

