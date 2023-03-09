use std::env;
use std::net::IpAddr;

use torrust_tracker_configuration::Configuration;
use torrust_tracker_primitives::TrackerMode;

use crate::random;

/// This configuration is used for testing. It generates random config values so they do not collide
/// if you run more than one tracker at the same time.
///
/// # Panics
///
/// Will panic if it can't convert the temp file path to string
#[must_use]
pub fn ephemeral() -> Configuration {
    // todo: disable services that are not needed.
    // For example: a test for the UDP tracker should disable the API and HTTP tracker.

    let mut config = Configuration {
        log_level: Some("off".to_owned()), // Change to `debug` for tests debugging
        ..Default::default()
    };

    // Ephemeral socket address for API
    let api_port = 0u16;
    config.http_api.enabled = true;
    config.http_api.bind_address = format!("127.0.0.1:{}", &api_port);

    // Ephemeral socket address for UDP tracker
    let udp_port = 0u16;
    config.udp_trackers[0].enabled = true;
    config.udp_trackers[0].bind_address = format!("127.0.0.1:{}", &udp_port);

    // Ephemeral socket address for HTTP tracker
    let http_port = 0u16;
    config.http_trackers[0].enabled = true;
    config.http_trackers[0].bind_address = format!("127.0.0.1:{}", &http_port);

    // Ephemeral sqlite database
    let temp_directory = env::temp_dir();
    let random_db_id = random::string(16);
    let temp_file = temp_directory.join(format!("data_{random_db_id}.db"));
    config.db_path = temp_file.to_str().unwrap().to_owned();

    config
}

#[must_use]
pub fn ephemeral_with_reverse_proxy() -> Configuration {
    let mut cfg = ephemeral();

    cfg.on_reverse_proxy = true;

    cfg
}

#[must_use]
pub fn ephemeral_without_reverse_proxy() -> Configuration {
    let mut cfg = ephemeral();

    cfg.on_reverse_proxy = false;

    cfg
}

#[must_use]
pub fn ephemeral_mode_public() -> Configuration {
    let mut cfg = ephemeral();

    cfg.mode = TrackerMode::Public;

    cfg
}

#[must_use]
pub fn ephemeral_mode_private() -> Configuration {
    let mut cfg = ephemeral();

    cfg.mode = TrackerMode::Private;

    cfg
}

#[must_use]
pub fn ephemeral_mode_whitelisted() -> Configuration {
    let mut cfg = ephemeral();

    cfg.mode = TrackerMode::Listed;

    cfg
}

#[must_use]
pub fn ephemeral_mode_private_whitelisted() -> Configuration {
    let mut cfg = ephemeral();

    cfg.mode = TrackerMode::PrivateListed;

    cfg
}

#[must_use]
pub fn ephemeral_with_external_ip(ip: IpAddr) -> Configuration {
    let mut cfg = ephemeral();

    cfg.external_ip = Some(ip.to_string());

    cfg
}

#[must_use]
pub fn ephemeral_ipv6() -> Configuration {
    let mut cfg = ephemeral();

    let ipv6 = format!("[::]:{}", 0);

    cfg.http_api.bind_address = ipv6.clone();
    cfg.http_trackers[0].bind_address = ipv6.clone();
    cfg.udp_trackers[0].bind_address = ipv6;

    cfg
}
