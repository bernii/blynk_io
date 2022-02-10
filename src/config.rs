use log::*;

#[derive(Debug)]
pub struct Config {
    pub token: String,
    pub server: String,
    pub port: u64,
}

impl Config {
    pub fn new<T>(mut args: T) -> Result<Self, &'static str>
    where
        T: Iterator<Item = String>,
    {
        args.next();

        let token = match args.next() {
            Some(arg) => arg,
            None => return Err("Token not provided"),
        };

        let server = match args.next() {
            Some(arg) => arg,
            None => {
                let server = "blynk-cloud.com";
                info!("No server name provided, using default ({})", server);
                server.into()
            }
        };

        let port = match args.next() {
            Some(arg) => arg.parse::<u64>().unwrap(),
            None => {
                let port = 80u64;
                info!("No server name provided, using default ({})", port);
                port
            }
        };

        Ok(Config {
            token,
            server,
            port,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_required() {
        let args = ["progname"].iter().map(|s| s.to_string());
        let result = Config::new(args).unwrap_err();
        assert_eq!("Token not provided", result);
    }

    #[test]
    fn server_and_port_parsed() {
        let server = "example.com";
        let port = "1234";
        let vec = vec!["pogname", "token", server, port];
        let args = vec.iter().map(|s| s.to_string());
        let conf = Config::new(args).unwrap();
        assert_eq!(server, conf.server);
        assert_eq!(port.parse::<u64>().unwrap(), conf.port);
    }

    #[test]
    fn server_and_port_default() {
        let args = ["progname", "token"].iter().map(|s| s.to_string());
        let conf = Config::new(args).unwrap();
        assert_eq!("token", conf.token);
        assert_eq!("blynk-cloud.com", conf.server);
        assert_eq!(80, conf.port);
    }
}
