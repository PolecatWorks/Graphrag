use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, PartialEq)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Sets the configuration file path
    #[arg(short, long, value_name = "FILE")]
    pub config: PathBuf,

    /// Sets directory where secrets are mounted (e.g. /app/secrets)
    #[arg(short, long, value_name = "DIR")]
    pub secrets_dir: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_missing_config_fails() {
        let result = Cli::try_parse_from(["app"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_short_config_flag() {
        let cli = Cli::try_parse_from(["app", "-c", "config.yaml"]).unwrap();
        assert_eq!(
            cli,
            Cli {
                config: PathBuf::from("config.yaml"),
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
                config: PathBuf::from("custom.yaml"),
                secrets_dir: None
            }
        );
    }

    #[test]
    fn test_cli_secrets_dir_flags() {
        let cli_short =
            Cli::try_parse_from(["app", "-c", "config.yaml", "-s", "/app/secrets"]).unwrap();
        assert_eq!(
            cli_short,
            Cli {
                config: PathBuf::from("config.yaml"),
                secrets_dir: Some(PathBuf::from("/app/secrets"))
            }
        );

        let cli_long = Cli::try_parse_from([
            "app",
            "--config",
            "config.yaml",
            "--secrets-dir",
            "/app/secrets",
        ])
        .unwrap();
        assert_eq!(
            cli_long,
            Cli {
                config: PathBuf::from("config.yaml"),
                secrets_dir: Some(PathBuf::from("/app/secrets"))
            }
        );
    }

    #[test]
    fn test_cli_unknown_flag_fails() {
        let result = Cli::try_parse_from(["app", "-c", "config.yaml", "--unknown-flag"]);
        assert!(result.is_err());
    }
}

