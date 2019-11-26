pub mod fip_client;
use std::time::{Duration, SystemTime};

use rspotify::spotify::client::Spotify;
use rspotify::spotify::oauth2::{SpotifyClientCredentials, SpotifyOAuth};
use rspotify::spotify::util::get_token;

pub fn fetch_last_songs(dur: Duration) -> Vec<fip_client::TimelineItem> {
    let from = SystemTime::now();
    let mut current = from.duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let until = current - dur;
    let mut res: Vec<fip_client::TimelineItem> = Vec::new();
    let max_pages = 2;
    let mut itrs = 0;
    loop {
        let t = SystemTime::UNIX_EPOCH + current;
        log::info!("Fetching page {} of logs at time {:?}", itrs, t);
        let (mut ss, page) = fip_client::fetch_songs(t).unwrap();
        log::info!("Fetched {} elements. Page info is {:?}", ss.len(), page);
        res.append(ss.as_mut());
        let end_string = String::from_utf8(base64::decode(&page.end_cursor).unwrap()).unwrap();
        let end_sec_u = u64::from_str_radix(end_string.as_str(), 10).unwrap();
        let end_sec = Duration::from_secs(end_sec_u);
        current = end_sec;
        if itrs >= max_pages || !page.has_next_page || end_sec < until {
            log::info!(
                "Fetched {} songs. Started at: {}, end_sec: {}, until: {}",
                res.len(),
                humantime::format_rfc3339(from),
                humantime::format_rfc3339(SystemTime::UNIX_EPOCH + end_sec),
                humantime::format_rfc3339(SystemTime::UNIX_EPOCH + until)
            );
            break;
        }
        itrs = itrs + 1;
    }
    res
}

fn update_playlist() {
    log::info!("Updating playlist");

    // Set client_id and client_secret in .env file or
    // export CLIENT_ID="your client_id"
    // export CLIENT_SECRET="secret"
    // export REDIRECT_URI=your-direct-uri
    let mut oauth = SpotifyOAuth::default()
        .scope("playlist-modify-private playlist-modify-public")
        .build();

    match get_token(&mut oauth) {
        Some(token_info) => {
            let client_credential = SpotifyClientCredentials::default()
                .token_info(token_info)
                .build();

            let spotify = Spotify::default()
                .client_credentials_manager(client_credential)
                .build();

            let user_id = "KZ-2BPJ0Tum-W8n2kB5d8A";
            let mut playlist_id = String::from("4Qghjo06iuI9rhqtzE4Ved?si=QJhqsgPxSwybqYVWEBvRQg");
            let playlist = spotify
                .user_playlist(user_id, Some(&mut playlist_id), None, None)
                .unwrap();
            log::info!("Found playlist {:?}", playlist);
            // spotify.user_playlist_add_tracks(user_id, playlist_id: &str, track_ids: &[String], position: Option<i32>)
        }
        None => panic!("Spotify auth failed"),
    };
}

fn main() {
    env_logger::init();
    // let oneh = 60 * 60; // 1 hour
    // let d = Duration::from_secs(oneh * 10);
    // let songs = fetch_last_songs(d);
    update_playlist();
    // for s in &songs {
    //     println!("{:?}", *s);
    // }
}
