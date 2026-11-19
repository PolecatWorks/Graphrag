use clap::Parser;

#[derive(Parser, Debug, PartialEq)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Sets a custom config file
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<String>,

    /// Sets directory where secrets are mounted (e.g. /app/secrets)
    #[arg(short, long, value_name = "DIR")]
    pub secrets_dir: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_default_no_args() {
        let cli = Cli::try_parse_from(["app"]).unwrap();
        assert_eq!(
            cli,
            Cli {
                config: None,
                secrets_dir: None
            }
        );
    }

    #[test]
    fn test_cli_short_config_flag() {
        let cli = Cli::try_parse_from(["app", "-c", "config.yaml"]).unwrap();
        assert_eq!(
            cli,
            Cli {
                config: Some("config.yaml".to_string()),
                secrets_dir: None
            }
        );
    }

    #[test]
    fn test_cli_long_config_flag() {
        let cli = Cli::try_parse_from(["app", "--config", "custom.yaml"]).unwrap();
        assert_eq!(
            cli,
            Cli {
                config: Some("custom.yaml".to_string()),
                secrets_dir: None
            }
        );
    }

    #[test]
    fn test_cli_secrets_dir_flags() {
        let cli_short = Cli::try_parse_from(["app", "-s", "/app/secrets"]).unwrap();
        assert_eq!(
            cli_short,
            Cli {
                config: None,
                secrets_dir: Some("/app/secrets".to_string())
            }
        );

        let cli_long = Cli::try_parse_from(["app", "--secrets-dir", "/app/secrets"]).unwrap();
        assert_eq!(
            cli_long,
            Cli {
                config: None,
                secrets_dir: Some("/app/secrets".to_string())
            }
        );
    }

    #[test]
    fn test_cli_unknown_flag_fails() {
        let result = Cli::try_parse_from(["app", "--unknown-flag"]);
        assert!(result.is_err());
    }
}

