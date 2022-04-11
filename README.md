<div id="top"></div>
<!--
*** Thanks for checking out the Best-README-Template. If you have a suggestion
*** that would make this better, please fork the repo and create a pull request
*** or simply open an issue with the tag "enhancement".
*** Don't forget to give the project a star!
*** Thanks again! Now go create something AMAZING! :D
-->



<!-- PROJECT SHIELDS -->
<!--
*** I'm using markdown "reference style" links for readability.
*** Reference links are enclosed in brackets [ ] instead of parentheses ( ).
*** See the bottom of this document for the declaration of the reference variables
*** for contributors-url, forks-url, etc. This is an optional, concise syntax you may use.
*** https://www.markdownguide.org/basic-syntax/#reference-style-links
-->
[![Contributors][contributors-shield]][contributors-url]
[![Forks][forks-shield]][forks-url]
[![Stargazers][stars-shield]][stars-url]
[![Issues][issues-shield]][issues-url]
[![Build Status][build-status]][build-status-url]
[![MIT License][license-shield]][license-url]
[![LinkedIn][linkedin-shield]][linkedin-url]



<!-- PROJECT LOGO -->
<br />
<div align="center">
  <a href="https://github.com/bernii/blynk_io">
    <img src="https://upload.wikimedia.org/wikipedia/commons/d/d5/Rust_programming_language_black_logo.svg" alt="rust logo" width="80" height="80">
  </a>

<h3 align="center">blynk.io@rust</h3>

  <p align="center">
    Blynk.io Integration in Rust
    <br />
    <a href="https://github.com/bernii/blynk_io"><strong>Explore the docs »</strong></a>
    <br />
    <br />
    <a href="https://crates.io/crates/blynk_io">Rust Crate</a>
    ·
    <a href="https://github.com/bernii/blynk_io/issues">Report Bug</a>
    ·
    <a href="https://github.com/bernii/blynk_io/issues">Request Feature</a>
  </p>
</div>



<!-- TABLE OF CONTENTS -->
<details>
  <summary>Table of Contents</summary>
  <ol>
    <li>
      <a href="#about-the-project">About The Project</a>
      <ul>
        <li><a href="#built-with">Built With</a></li>
      </ul>
    </li>
    <li>
      <a href="#getting-started">Getting Started</a>
      <ul>
        <li><a href="#prerequisites">Prerequisites</a></li>
        <li><a href="#installation">Installation</a></li>
      </ul>
    </li>
    <li><a href="#usage">Usage</a></li>
    <li><a href="#roadmap">Roadmap</a></li>
    <li><a href="#license">License</a></li>
    <li><a href="#contact">Contact</a></li>
    <li><a href="#acknowledgments">Acknowledgments</a></li>
  </ol>
</details>



<!-- ABOUT THE PROJECT -->
## About The Project

This library was created with intent to support Rust usage and prototpying on ESP32 boards with help of [esp-rs](https://github.com/esp-rs) project that enables use of Rust on various SoCs.

The project was mainly based on the official [python implementation](https://github.com/blynkkk/lib-python) since there are no extensive docs of the API.

<p align="right">(<a href="#top">back to top</a>)</p>



### Built With

* [rust](https://nextjs.org/)
* [restruct_derive](https://lib.rs/crates/restruct_derive)

<p align="right">(<a href="#top">back to top</a>)</p>



<!-- GETTING STARTED -->
## Getting Started

Make sure you have your `rust` environment configurated

### Installation

1. Add library to your `Cargo.toml`

    ```toml
    ...
    [dependencies]
    blynk_io = "0.3.0"
    ```
2. Use the library in you code
    ```rust
    use blynk_io::*;
    ...
    let mut blynk = <Blynk>::new("AUTH_TOKEN".to_string());

    fn main() {
        loop {
            blynk.run();
            thread::sleep(Duration::from_millis(50));
        }
    }
    ```
3. Have fun! :relieved:

<p align="right">(<a href="#top">back to top</a>)</p>



<!-- USAGE EXAMPLES -->
## Usage

1. Get an AUTH_TOKEN Key with [https://blynk.io](https://blynk.io) app
2. Install cargo binary crate to be able to test easily on your computer
    ```bash
    $ cargo install blynk_io
    ```
3. Run the provided binary example with provided `AUTH_TOKEN`
   ```bash
   $ blynk_io --features build-binary AUTH_TOKEN
   ```
   (**Optional**) if you want to run the client in async mode, start the folllowing
   example
   ```bash
   $ blynk_io --features build-binary,async AUTH_TOKEN
   ```
4. You should see an output similar to the followig one
    ```log
    2022-02-10T16:24:27.352Z INFO [blynk_io::config] No server name provided, using default (blynk-cloud.com)
    2022-02-10T16:24:27.353Z INFO [blynk_io::config] No server name provided, using default (80)
    Using auth token for G7HDmT7fraqB5A6WOautoTCQ0XvRplCv
    Connecting to blynk-cloud.com:80
    2022-02-10T16:24:27.353Z ERROR [blynk_io] Not connected, trying reconnect
    2022-02-10T16:24:27.419Z INFO [blynk_io] Successfully connected to blynk server
    2022-02-10T16:24:27.419Z INFO [blynk_io] Authenticating device...
    Sent message, awaiting reply...!!
    2022-02-10T16:24:27.449Z DEBUG [blynk_io::client] size (5) vs consumed (5)
    2022-02-10T16:24:27.449Z DEBUG [blynk_io::client] Got response message: Message { mtype: Rsp, id: 1, size: None, status: Some(StatusOk), body: [] }
    2022-02-10T16:24:27.449Z INFO [blynk_io] Access granted
    2022-02-10T16:24:27.449Z INFO [blynk_io] Setting heartbeat
    Sent message, awaiting reply...!!
    2022-02-10T16:24:27.479Z DEBUG [blynk_io::client] size (5) vs consumed (5)
    2022-02-10T16:24:27.479Z DEBUG [blynk_io::client] Got response message: Message { mtype: Rsp, id: 2, size: None, status: Some(StatusOk), body: [] }
    ```

<p align="right">(<a href="#top">back to top</a>)</p>



<!-- ROADMAP -->
## Roadmap

- [x] add tests
- [x] add rustdocs
- [x] CI integration with GithHub Actions
- [x] better error generation & handling
- [x] add async support once it's stable in esp-rs
- [ ] better test coverage
- [ ] ssl implementation

See the [open issues](https://github.com/bernii/blynk_io/issues) for a full list of proposed features (and known issues).

<p align="right">(<a href="#top">back to top</a>)</p>



<!-- LICENSE -->
## License

Distributed under the MIT License. See `LICENSE` for more information.

<p align="right">(<a href="#top">back to top</a>)</p>



<!-- CONTACT -->
## Contact

Bernard Kobos - [@bkobos](https://twitter.com/bkobos) - bkobos@gmail.com

Project Link: [https://github.com/bernii/blynk_io](https://github.com/bernii/blynk_io)

<p align="right">(<a href="#top">back to top</a>)</p>



<!-- ACKNOWLEDGMENTS -->
## Acknowledgments

* great [Ivan Markov](https://github.com/ivmarkov) work and help
* extremely helpful esp-rs [community](https://app.element.io/#/room/#esp-rs:matrix.org)
* Ivan's [demo](https://github.com/ivmarkov/rust-esp32-std-demo) which is a great starting point
* ESP-RS [book](https://esp-rs.github.io/book/)
* Blynk [Python Library](https://github.com/blynkkk/lib-python)

<p align="right">(<a href="#top">back to top</a>)</p>



<!-- MARKDOWN LINKS & IMAGES -->
<!-- https://www.markdownguide.org/basic-syntax/#reference-style-links -->
[contributors-shield]: https://img.shields.io/github/contributors/bernii/blynk_io.svg?style=for-the-badge
[contributors-url]: https://github.com/bernii/blynk_io/graphs/contributors
[forks-shield]: https://img.shields.io/github/forks/bernii/blynk_io.svg?style=for-the-badge
[forks-url]: https://github.com/bernii/blynk_io/network/members
[stars-shield]: https://img.shields.io/github/stars/bernii/blynk_io.svg?style=for-the-badge
[stars-url]: https://github.com/bernii/blynk_io/stargazers
[issues-shield]: https://img.shields.io/github/issues/bernii/blynk_io.svg?style=for-the-badge
[issues-url]: https://github.com/bernii/blynk_io/issues
[license-shield]: https://img.shields.io/github/license/bernii/blynk_io.svg?style=for-the-badge
[license-url]: https://github.com/bernii/blynk_io/blob/master/LICENSE
[linkedin-shield]: https://img.shields.io/badge/-LinkedIn-black.svg?style=for-the-badge&logo=linkedin&colorB=555
[linkedin-url]: https://linkedin.com/in/bernii
[product-screenshot]: images/screenshot.png
[build-status]: https://img.shields.io/endpoint.svg?url=https%3A%2F%2Factions-badge.atrox.dev%2Fbernii%2Fblynk_io%2Fbadge%3Fref%3Dmain&style=for-the-badge
[build-status-url]: https://actions-badge.atrox.dev/bernii/blynk_io/goto?ref=main
