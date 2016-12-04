//! User-facing configuration.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use base64::decode;
use iron::{Plugin, Request};
use iron::typemap::Key;
use persistent::Read as PersistentRead;
use serde_json;
use serde_yaml;
use toml;

use error::{ApiError, CliError};

/// The user's configuration as loaded from the configuration file.
///
/// Refer to `Config` for the description of the fields.
#[derive(Deserialize, RustcDecodable)]
struct RawConfig {
    bind_address: Option<String>,
    bind_port: Option<String>,
    domain: String,
    macaroon_secret_key: String,
    postgres_url: String,
}

/// Server configuration provided by the user.
#[derive(Clone)]
pub struct Config {
    /// The network address where the server should listen for connections. Defaults to 127.0.0.1.
    pub bind_address: String,
    /// The network port where the server should listen for connections. Defaults to 3000.
    pub bind_port: String,
    /// The DNS name where clients can reach the server. Used as the hostname portion of user IDs.
    pub domain: String,
    /// The secret key used for generating
    /// [Macaroons](https://research.google.com/pubs/pub41892.html). Must be 32
    /// cryptographically random bytes, encoded as a Base64 string. Changing this value will
    /// invalidate any previously generated macaroons.
    pub macaroon_secret_key: Vec<u8>,
    /// A [PostgreSQL connection string](http://www.postgresql.org/docs/current/static/libpq-connect.html#LIBPQ-CONNSTRING)
    /// for Ruma's PostgreSQL database.
    pub postgres_url: String,
}

impl Config {
    /// Load the user's configuration file.
    pub fn from_file(filename: Option<&str>) -> Result<Config, CliError> {
        let config: RawConfig;

        if let Some(filename) = filename {
            info!("Using custom filename: `{}`", filename);
            if Path::new(filename).is_file() {
                if filename.ends_with(".json") {
                    info!("Parsing JSON config file: `{}`", filename);
                    config = Self::load_json(filename)?;
                } else if filename.ends_with(".toml") {
                    info!("Parsing toml config file: `{}`", filename);
                    config = Self::load_toml(filename)?;
                } else if filename.ends_with(".yml") || filename.ends_with(".yaml") {
                    info!("Parsing yaml config file: `{}`", filename);
                    config = Self::load_yaml(filename)?;
                } else {
                    debug!("Failed to identify type of config file: `{}`", filename);
                    return Err(CliError::new("Could not recognize custom configuration file."));
                }
            } else {
                debug!("Couldn't find specified configuration file: `{}`", filename);
                return Err(CliError::new("User-specified configuration file was not found."));
            }
        } else if Self::json_exists() {
            info!("Parsing JSON config file: `ruma.json`");
            config = Self::load_json("ruma.json")?;
        } else if Self::toml_exists() {
            info!("Parsing TOML config file: `ruma.toml`");
            config = Self::load_toml("ruma.toml")?;
        } else if Self::yaml_exists() {
            let yaml_fn = if Path::new("ruma.yaml").is_file() {
                "ruma.yaml"
            } else {
                "ruma.yml"
            };
            info!("Parsing YAML config file: `{}`", yaml_fn);
            config = Self::load_yaml(yaml_fn)?;
        } else {
            error!("Couldn't find config file: `ruma.*`");
            return Err(CliError::new("No configuration file was found."));
        }

        let macaroon_secret_key = match decode(&config.macaroon_secret_key) {
            Ok(bytes) => match bytes.len() {
                32 => bytes,
                _ => {
                    debug!("Found secret key of invalid length");
                    return Err(CliError::new("macaroon_secret_key must be 32 bytes."))
                },
            },
            Err(e) => {
                debug!("Failed to retrieve macaroon secret {}", e);
                return Err(CliError::new(
                "macaroon_secret_key must be valid Base64."
            ))},
        };
        
        let address = match config.bind_address {
            Some(a) => {
                info!("Parsed address to use: {}", a);
                a
            },
            None => {
                info!("Failed to locate address; defaulting to localhost");
                String::from("127.0.0.1")
            }
        };
        let port = match config.bind_port {
            Some(p) => {
                info!("Parsed port to use: {}", p);
                p
            },
            None => {
                info!("Failed to locate port; defaulting to 3000");
                String::from("3000")
            }
        };

        Ok(Config {
            bind_address: address,
            bind_port: port,
            domain: config.domain,
            macaroon_secret_key: macaroon_secret_key,
            postgres_url: config.postgres_url,
        })
    }

    /// Load the `RawConfig` from a JSON configuration file.
    fn load_json(filename: &str) -> Result<RawConfig, CliError> {
        let contents = Self::read_file_contents(filename);
        match serde_json::from_str(&contents) {
            Ok(config) => {
                info!("Successfully parsed JSON config file");
                return Ok(config)
            },
            Err(error) => {
                debug!("Failed to parse JSON config file: {}", error);
                return Err(CliError::from(error))
            },
        };
    }

    /// Load the `RawConfig` from a TOML configuration file.
    fn load_toml(filename: &str) -> Result<RawConfig, CliError> {
        let contents = Self::read_file_contents(filename);
        let mut parser = toml::Parser::new(&contents);
        let data  = parser.parse();

        if data.is_none() {
            for err in &parser.errors {
                let (loline, locol) = parser.to_linecol(err.lo);
                let (hiline, hicol) = parser.to_linecol(err.hi);
                println!("{}: {}:{}-{}:{} error: {}", filename, loline, locol, hiline, hicol, err.desc);
                debug!("{}: {}:{}-{}:{} error: {}", filename, loline, locol, hiline, hicol, err.desc);
            }

            return Err(CliError::new("Unable to parse toml config file."));
        }

        let config = toml::Value::Table(data.unwrap());
        match toml::decode(config) {
            Some(t) => {
                info!("Successfully parsed `{}`", filename);
                return Ok(t)
            },
            None => {
                debug!("Failed to retrieve valid information in the decode phase from `{}`",
                       filename);
                return Err(CliError::new("Error while decoding toml config file."))
            },
        }
    }

    /// Load the `RawConfig` from a YAML configuration file.
    fn load_yaml(filename: &str) -> Result<RawConfig, CliError> {
        let contents;

        contents = Self::read_file_contents(filename);

        match serde_yaml::from_str(&contents) {
            Ok(config) => return Ok(config),
            Err(error) => return Err(CliError::from(error)),
        };
    }

    /// Read the contents of a file.
    fn read_file_contents(path: &str) -> String {
        let mut file = File::open(path).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        contents
    }

    /// Check if there is a configuration file in JSON.
    fn json_exists() -> bool {
        Path::new("ruma.json").is_file()
    }

    /// Check if there is a configuration file in TOML.
    fn toml_exists() -> bool {
        Path::new("ruma.toml").is_file()
    }

    /// Check if there is a configuration file in YAML.
    fn yaml_exists() -> bool {
        Path::new("ruma.yml").is_file() || Path::new("ruma.yaml").is_file()
    }

    /// Extract the `Config` stored in the request.
    pub fn from_request(request: &mut Request) -> Result<Arc<Config>, ApiError> {
        request.get::<PersistentRead<Config>>().map_err(ApiError::from)
    }
}

impl Key for Config {
    type Value = Config;
}
