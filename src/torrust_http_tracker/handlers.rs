use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use log::debug;
use warp::{reject, Rejection, Reply};
use warp::http::{Response};
use crate::{InfoHash, TorrentError, TorrentPeer, TorrentStats, TorrentTracker};
use crate::key_manager::AuthKey;
use crate::torrust_http_tracker::{AnnounceRequest, AnnounceResponse, ErrorResponse, Peer, ScrapeRequest, ScrapeResponse, ScrapeResponseEntry, ServerError, WebResult};
use crate::utils::url_encode_bytes;

/// Authenticate InfoHash using optional AuthKey
pub async fn authenticate(info_hash: &InfoHash, auth_key: &Option<AuthKey>, tracker: Arc<TorrentTracker>) -> Result<(), ServerError> {
    match tracker.authenticate_request(info_hash, auth_key).await {
        Ok(_) => Ok(()),
        Err(e) => {
            let err = match e {
                TorrentError::TorrentNotWhitelisted => ServerError::TorrentNotWhitelisted,
                TorrentError::PeerNotAuthenticated => ServerError::PeerNotAuthenticated,
                TorrentError::PeerKeyNotValid => ServerError::PeerKeyNotValid,
                TorrentError::NoPeersFound => ServerError::NoPeersFound,
                TorrentError::CouldNotSendResponse => ServerError::InternalServerError,
                TorrentError::InvalidInfoHash => ServerError::InvalidInfoHash,
            };

            Err(err)
        }
    }
}

/// Handle announce request
pub async fn handle_announce(announce_request: AnnounceRequest, auth_key: Option<AuthKey>, tracker: Arc<TorrentTracker>) -> WebResult<impl Reply> {
    if let Err(e) = authenticate(&announce_request.info_hash, &auth_key, tracker.clone()).await {
        return Err(reject::custom(e))
    }

    debug!("{:?}", announce_request);

    if tracker.config.on_reverse_proxy && announce_request.forwarded_ip.is_none() {
        return Err(reject::custom(ServerError::AddressNotFound))
    }

    let peer_ip = match tracker.config.on_reverse_proxy {
        true => announce_request.forwarded_ip.unwrap(),
        false => announce_request.peer_addr.ip()
    };

    let peer = TorrentPeer::from_http_announce_request(&announce_request, peer_ip, tracker.config.get_ext_ip());
    let torrent_stats = tracker.update_torrent_with_peer_and_get_stats(&announce_request.info_hash, &peer).await;
    // get all peers excluding the client_addr
    let peers = tracker.get_torrent_peers(&announce_request.info_hash, &peer.peer_addr).await;
    //if peers.is_none() { return Err(reject::custom(ServerError::NoPeersFound)) }

    // success response
    let tracker_copy = tracker.clone();
    tokio::spawn(async move {
        let mut status_writer = tracker_copy.set_stats().await;
        if peer_ip.is_ipv4() {
            status_writer.tcp4_connections_handled += 1;
            status_writer.tcp4_announces_handled += 1;
        } else {
            status_writer.tcp6_connections_handled += 1;
            status_writer.tcp6_announces_handled += 1;
        }
    });
    let announce_interval = tracker.config.announce_interval;
    send_announce_response(&announce_request, torrent_stats, peers, announce_interval)
}

/// Handle scrape request
pub async fn handle_scrape(scrape_request: ScrapeRequest, auth_key: Option<AuthKey>, tracker: Arc<TorrentTracker>) -> WebResult<impl Reply> {
    let mut files: HashMap<String, ScrapeResponseEntry> = HashMap::new();
    let db = tracker.get_torrents().await;

    for info_hash in scrape_request.info_hashes.iter() {
        // authenticate every info_hash
        if authenticate(info_hash, &auth_key, tracker.clone()).await.is_err() { continue }

        let scrape_entry = match db.get(&info_hash) {
            Some(torrent_info) => {
                let (seeders, completed, leechers) = torrent_info.get_stats();
                ScrapeResponseEntry { complete: seeders, downloaded: completed, incomplete: leechers }
            }
            None => {
                ScrapeResponseEntry { complete: 0, downloaded: 0, incomplete: 0 }
            }
        };

        if let Ok(encoded_info_hash) = url_encode_bytes(&info_hash.0) {
            files.insert(encoded_info_hash, scrape_entry);
        }
    }

    let ip = match tracker.config.on_reverse_proxy {
        true => scrape_request.forwarded_ip.unwrap(),
        false => scrape_request.remote_addr.ip()
    };

    let tracker_copy = tracker.clone();
    tokio::spawn(async move {
        let mut status_writer = tracker_copy.set_stats().await;
        if ip.is_ipv4() {
            status_writer.tcp4_connections_handled += 1;
            status_writer.tcp4_scrapes_handled += 1;
        } else {
            status_writer.tcp6_connections_handled += 1;
            status_writer.tcp6_scrapes_handled += 1;
        }
    });
    send_scrape_response(files)
}

/// Handle all server errors and send error reply
pub async fn handle_error(r: Rejection) -> std::result::Result<impl Reply, Infallible> {
    if let Some(e) = r.find::<ServerError>() {
        debug!("{:?}", e);
        let body: String = ErrorResponse { failure_reason: e.to_string() }.write();
        Ok(Response::new(body))
    } else {
        let body: String = ErrorResponse { failure_reason: "internal server error".to_string() }.write();
        Ok(Response::new(body))
    }
}

/// Send announce response
fn send_announce_response(announce_request: &AnnounceRequest, torrent_stats: TorrentStats, peers: Vec<TorrentPeer>, interval: u32) -> WebResult<impl Reply> {
    let http_peers: Vec<Peer> = peers.iter().map(|peer| Peer {
        peer_id: peer.peer_id.to_string(),
        ip: peer.peer_addr.ip(),
        port: peer.peer_addr.port()
    }).collect();

    let res = AnnounceResponse {
        interval,
        complete: torrent_stats.seeders,
        incomplete: torrent_stats.leechers,
        peers: http_peers
    };

    // check for compact response request
    if let Some(1) = announce_request.compact {
        match res.write_compact() {
            Ok(body) => Ok(Response::new(body)),
            Err(_) => Err(reject::custom(ServerError::InternalServerError))
        }
    } else {
        Ok(Response::new(res.write().into()))
    }
}

/// Send scrape response
fn send_scrape_response(files: HashMap<String, ScrapeResponseEntry>) -> WebResult<impl Reply> {
    Ok(Response::new(ScrapeResponse { files }.write()))
}
