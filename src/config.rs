use std::io::{Read, Write};
use std::fs::File;
use std::path::{Path, PathBuf};

use rustc_serialize::json;

/// Given a `home_dir` (e.g. from `std::env::home_dir()`), returns the default
/// location of the client configuration file,
/// `$HOME/.puppetlabs/client-tools/puppetdb.conf`.
pub fn default_config_path(mut home_dir: PathBuf) -> String {
    home_dir.push(".puppetlabs");
    home_dir.push("client-tools");
    home_dir.push("puppetdb");
    home_dir.set_extension("conf");
    home_dir.to_str().unwrap().to_owned()
}

pub fn global_config_path() -> String {
    let mut path = PathBuf::from("/etc/puppetlabs/client-tools");
    path.push("puppetdb");
    path.set_extension("conf");
    path.to_str().unwrap().to_owned()
}

fn split_server_urls(urls: String) -> Vec<String> {
    urls.split(",").map(|u| u.trim().to_string()).collect()
}

#[test]
fn split_server_urls_works() {
    assert_eq!(vec!["http://localhost:8080".to_string(), "http://foo.bar.baz:9190".to_string()],
               split_server_urls("   http://localhost:8080  ,   http://foo.bar.baz:9190"
                                     .to_string()))
}

#[derive(RustcDecodable,RustcEncodable,Clone,Debug)]
pub struct Config {
    pub server_urls: Vec<String>,
    pub cacert: Option<String>,
    pub cert: Option<String>,
    pub key: Option<String>,
    pub token: Option<String>,
}

pub fn merge_configs(first: PdbConfigSection, second: PdbConfigSection) -> PdbConfigSection {
    PdbConfigSection {
        server_urls: second.server_urls.or(first.server_urls),
        cacert: second.cacert.or(first.cacert),
        cert: second.cert.or(first.cert),
        key: second.key.or(first.key),
    }
}

impl Config {
    pub fn load(path: String,
                urls: Option<String>,
                cacert: Option<String>,
                cert: Option<String>,
                key: Option<String>,
                token: Option<String>)
                -> Config {

        let server_urls = urls.and_then(|s| {
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            })
            .and_then(|s| Some(split_server_urls(s)));

        // TODO Don't parse config if urls aren't HTTP. This is trivial but it
        // would be best to merge the other auth validation code when
        // constructing the client with this.
        if server_urls.is_some() && cacert.is_some() && cert.is_some() && key.is_some() {
            return Config {
                server_urls: server_urls.unwrap(),
                cacert: cacert,
                cert: cert,
                key: key,
                token: None,
            };
        }

        let file_configs = merge_configs(PdbConfigSection::load(global_config_path()),
                                         PdbConfigSection::load(path));
        let flags_config = PdbConfigSection {
            server_urls: server_urls,
            cacert: cacert,
            cert: cert,
            key: key,
        };
        let cfg = merge_configs(file_configs, flags_config);

        // TODO Add tests for Config parsing edge cases
        Config {
            server_urls: cfg.server_urls.unwrap_or(default_server_urls()),
            cacert: cfg.cacert,
            cert: cfg.cert,
            key: cfg.key,
            token: token,
        }
    }
}

#[derive(RustcDecodable,RustcEncodable,Debug)]
pub struct PdbConfigSection {
    server_urls: Option<Vec<String>>,
    cacert: Option<String>,
    cert: Option<String>,
    key: Option<String>,
}

fn default_server_urls() -> Vec<String> {
    vec!["http://127.0.0.1:8080".to_string()]
}

fn empty_pdb_config_section() -> PdbConfigSection {
    PdbConfigSection {
        server_urls: None,
        cacert: None,
        cert: None,
        key: None,
    }
}

impl PdbConfigSection {
    fn load(path: String) -> PdbConfigSection {
        if !Path::new(&path).exists() {
            return empty_pdb_config_section();
        }
        let mut f = File::open(&path)
            .unwrap_or_else(|e| pretty_panic!("Error opening config {:?}: {}", path, e));
        let mut s = String::new();
        if let Err(e) = f.read_to_string(&mut s) {
            pretty_panic!("Error reading from config {:?}: {}", path, e)
        }
        let json = json::Json::from_str(&s)
            .unwrap_or_else(|e| pretty_panic!("Error parsing config {:?}: {}", path, e));
        PdbConfigSection {
            server_urls: match json.find_path(&["puppetdb", "server_urls"])
                .unwrap_or(&json::Json::Null) {
                &json::Json::Array(ref urls) => {
                    Some(urls.into_iter()
                        .map(|url| url.as_string().unwrap().to_string())
                        .collect::<Vec<String>>())
                }
                &json::Json::String(ref urls) => Some(split_server_urls(urls.clone())),
                &json::Json::Null => None,
                _ => {
                    pretty_panic!("Error parsing config {:?}: server_urls must be an Array or a \
                                   String",
                                  path)
                }
            },
            cacert: json.find_path(&["puppetdb", "cacert"])
                .and_then(|s| s.as_string().and_then(|s| Some(s.to_string()))),
            cert: json.find_path(&["puppetdb", "cert"])
                .and_then(|s| s.as_string().and_then(|s| Some(s.to_string()))),
            key: json.find_path(&["puppetdb", "key"])
                .and_then(|s| s.as_string().and_then(|s| Some(s.to_string()))),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs::File;
    use rustc_serialize::json;
    use std::io::{Write, Error};
    use std::path::PathBuf;

    extern crate tempdir;
    use self::tempdir::*;

    fn create_temp_path(temp_dir: &TempDir, file_name: &str) -> PathBuf {
        temp_dir.path().join(file_name)
    }

    #[derive(RustcEncodable)]
    struct CLIConfig {
        puppetdb: PdbConfigSection,
    }


    fn spit_config(file_path: &str, config: &CLIConfig) -> Result<(), Error> {
        let mut f = try!(File::create(file_path));
        try!(f.write_all(json::encode(config).unwrap().as_bytes()));
        Ok(())
    }

    #[test]
    fn load_test_all_fields() {
        let config = CLIConfig {
            puppetdb: PdbConfigSection {
                server_urls: Some(vec!["http://foo".to_string()]),
                cacert: Some("foo".to_string()),
                cert: Some("bar".to_string()),
                key: Some("baz".to_string()),
            },
        };

        let temp_dir = TempDir::new_in("target", "test-").unwrap();
        let temp_path = create_temp_path(&temp_dir, "testfile.json");
        let path_str = temp_path.as_path().to_str().unwrap();

        spit_config(path_str, &config).unwrap();
        let slurped_config = Config::load(path_str.to_string(), None, None, None, None, None);

        let PdbConfigSection { server_urls, cacert, cert, key } = config.puppetdb;
        assert_eq!(server_urls.unwrap()[0], slurped_config.server_urls[0]);
        assert_eq!(cacert, slurped_config.cacert);
        assert_eq!(cert, slurped_config.cert);
        assert_eq!(key, slurped_config.key)
    }

    fn spit_string(file_path: &str, contents: &str) -> Result<(), Error> {
        let mut f = try!(File::create(file_path));
        try!(f.write_all(contents.as_bytes()));
        Ok(())
    }

    #[test]
    fn load_test_only_urls_vector() {
        let temp_dir = TempDir::new_in("target", "test-").unwrap();
        let temp_path = create_temp_path(&temp_dir, "testfile.json");
        let path_str = temp_path.as_path().to_str().unwrap();

        spit_string(&path_str,
                    "{\"puppetdb\":{\"server_urls\":[\"http://foo\"]}}")
            .unwrap();
        let slurped_config = Config::load(path_str.to_string(), None, None, None, None, None);

        assert_eq!("http://foo", slurped_config.server_urls[0]);
        assert_eq!(None, slurped_config.cacert);
        assert_eq!(None, slurped_config.cert);
        assert_eq!(None, slurped_config.key);
    }

    #[test]
    fn load_test_only_urls_string() {
        let temp_dir = TempDir::new_in("target", "test-").unwrap();
        let temp_path = create_temp_path(&temp_dir, "testfile.json");
        let path_str = temp_path.as_path().to_str().unwrap();

        spit_string(&path_str,
                    "{\"puppetdb\":{\"server_urls\":\"http://foo,https://localhost:8080\"}}")
            .unwrap();
        let slurped_config = Config::load(path_str.to_string(), None, None, None, None, None);

        assert_eq!(vec!["http://foo", "https://localhost:8080"],
                   slurped_config.server_urls);
        assert_eq!(None, slurped_config.cacert);
        assert_eq!(None, slurped_config.cert);
        assert_eq!(None, slurped_config.key);
    }

    #[test]
    fn load_test_only_urls_null() {
        let temp_dir = TempDir::new_in("target", "test-").unwrap();
        let temp_path = create_temp_path(&temp_dir, "testfile.json");
        let path_str = temp_path.as_path().to_str().unwrap();

        spit_string(&path_str, "{\"puppetdb\":{\"server_urls\":null}}").unwrap();
        let slurped_config = Config::load(path_str.to_string(), None, None, None, None, None);

        assert_eq!(vec!["http://127.0.0.1:8080"], slurped_config.server_urls);
        assert_eq!(None, slurped_config.cacert);
        assert_eq!(None, slurped_config.cert);
        assert_eq!(None, slurped_config.key);
    }
}
