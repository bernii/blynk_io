use blynk_io::*;

use log::*;
#[cfg(feature = "build-binary")]
use simple_logger::SimpleLogger;
use std::time::Instant;
use std::{env, process};

struct EventsHandler {
    i: Instant,
}

#[cfg(feature = "async")]
use async_trait::async_trait;

#[cfg(feature = "async")]
#[async_trait]
impl Event for EventsHandler {
    async fn handle_vpin_read(&mut self, client: &mut Client, pin_num: u8) {
        info!("Wanting to read the state of pin {:?}", pin_num);
        match pin_num {
            5 => {
                client
                    .virtual_write(5, &format!("V5 {}", self.i.elapsed().as_secs()))
                    .await
                    .unwrap_or_default();
                info!("sent info about pin 5");
            }
            4 => {
                client
                    .virtual_write(4, &format!("V4 {}", self.i.elapsed().as_secs()))
                    .await
                    .unwrap_or_default();
                info!("sent info about pin 4");
            }
            pin => info!("pin not handled: v{}", pin),
        }
    }

    async fn handle_vpin_write(&mut self, _client: &mut Client, pin_num: u8, data: &str) {
        info!("Wanting to write the state of pin {:?} {:?}", pin_num, data);
    }
}

#[cfg(not(feature = "async"))]
impl Event for EventsHandler {
    fn handle_vpin_read(&mut self, client: &mut Client, pin_num: u8) {
        info!("Wanting to read the state of pin {:?}", pin_num);
        match pin_num {
            5 => {
                client
                    .virtual_write(5, &format!("V5 {}", self.i.elapsed().as_secs()))
                    .unwrap_or_default();
                info!("sent info about pin 5");
            }
            4 => {
                client
                    .virtual_write(4, &format!("V4 {}", self.i.elapsed().as_secs()))
                    .unwrap_or_default();
                info!("sent info about pin 4");
            }
            pin => info!("pin not handled: v{}", pin),
        }
    }

    fn handle_vpin_write(&mut self, _client: &mut Client, pin_num: u8, data: &str) {
        info!("Wanting to write the state of pin {:?} {:?}", pin_num, data);
    }
}

fn main() {
    SimpleLogger::new().init().unwrap();

    let config = Config::new(env::args()).unwrap_or_else(|err| {
        eprintln!("Problem parsing arguments: {}", err);
        process::exit(1);
    });

    println!("Using auth token for {}", config.token);
    println!("Connecting to {}:{}", config.server, config.port);

    let mut blynk = Blynk::new(config.token.clone());
    blynk.set_config(config);

    let mut handler = EventsHandler { i: Instant::now() };
    blynk.set_handler(handler);

    #[cfg(feature = "async")]
    smol::block_on(async {
        loop {
            blynk.run().await
        }
    });
    #[cfg(not(feature = "async"))]
    loop {
        blynk.run();
    }

    unreachable!("This code is not reachable ;-)");
}
