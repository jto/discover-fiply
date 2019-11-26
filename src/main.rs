pub mod fip_client;
use fip_client::TimelineItem;
use rspotify::spotify::client::Spotify;
use rspotify::spotify::oauth2::{SpotifyClientCredentials, SpotifyOAuth};
use rspotify::spotify::util::get_token;
use std::time::{Duration, SystemTime};

use retry::{delay, retry};

pub fn fetch_last_songs(dur: Duration) -> Vec<TimelineItem> {
    let from = SystemTime::now();
    let mut current = from.duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let until = current - dur;
    let mut res: Vec<TimelineItem> = Vec::new();
    let max_pages = 100;
    let mut itrs = 0;
    loop {
        let t = SystemTime::UNIX_EPOCH + current;
        log::info!("Fetching page {} of logs at time {:?}", itrs, t);
        let fip_call = retry(delay::Fixed::from_millis(100).take(3), || {
            fip_client::fetch_songs(t)
        });
        let (mut ss, page) = fip_call.unwrap();
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

///
/// Get the topN most popular songs
/// Popularity is the number of time occurs in {songs}
///
fn get_most_popular(songs: &mut Vec<TimelineItem>, limit: usize) -> Vec<TimelineItem> {
    log::info!(
        "Getting the most popular songs in a list of {}",
        songs.len()
    );
    songs.sort_by(|a, b| a.subtitle.cmp(&b.subtitle));
    let mut counted: Vec<(TimelineItem, u16)> = vec![];
    let mut last_seen = songs.pop().unwrap();
    let mut count = 1;
    for s in songs {
        if s.subtitle == last_seen.subtitle {
            count = count + 1;
        } else {
            counted.push((last_seen, count));
            last_seen = (*s).clone();
            count = 1;
        }
    }
    counted.sort_by_key(|p| p.1); // sort by most popular song
    let result: Vec<(TimelineItem, u16)> = counted.into_iter().rev().take(limit).collect();
    for p in &result {
        log::info!("- {} was played {} times", p.0.subtitle, p.1);
    }
    result.into_iter().map(|p| p.0).collect()
}

fn spotify_create_client() -> Spotify {
    // Set client_id and client_secret in .env
    let mut oauth = SpotifyOAuth::default()
        .scope("playlist-modify-private playlist-modify-public")
        .build();

    let spot = match get_token(&mut oauth) {
        Some(token_info) => {
            let client_credential = SpotifyClientCredentials::default()
                .token_info(token_info)
                .build();

            Spotify::default()
                .client_credentials_manager(client_credential)
                .build()
        }
        None => panic!("Spotify auth failed"),
    };

    spot
}

///
/// Find the spotify track IDS for each TimelineItem
///
fn find_tracks_ids(spotify: &Spotify, items: Vec<TimelineItem>) -> Vec<String> {
    let mut ids: Vec<String> = vec![];
    for t in &items {
        let q = format!(
            "track:{} artist:{}",
            &t.subtitle,
            &t.interpreters.first().unwrap()
        );
        log::debug!("Searching for track: {:?} with query {}", t, q);
        let track = spotify.search_track(&q, 1, 0, None).unwrap();
        log::debug!("Search result: {:?}", track);
        for f in &track.tracks.items.first() {
            for id in &f.id {
                ids.push(format!("spotify:track:{}", id));
            }
        }
    }
    ids
}

fn update_playlist(spotify: &Spotify, tracks: Vec<String>) {
    log::info!("Updating playlist with tracks: {:?}", tracks);

    let user_id = "KZ-2BPJ0Tum-W8n2kB5d8A";
    let mut playlist_id = String::from("4Qghjo06iuI9rhqtzE4Ved");
    let playlist = spotify
        .user_playlist(user_id, Some(&mut playlist_id), None, None)
        .unwrap();

    log::debug!("Found playlist {:?}", playlist);

    spotify
        .user_playlist_replace_tracks(user_id, playlist_id.as_str(), tracks.as_slice())
        .unwrap();
}

fn main() {
    env_logger::init();
    let a_day = 60 * 60 * 24;
    let d = Duration::from_secs(a_day * 7);
    let mut songs = fetch_last_songs(d);
    let populars = get_most_popular(songs.as_mut(), 125);
    let spotify = spotify_create_client();
    let tracks: Vec<String> = find_tracks_ids(&spotify, populars)
        .into_iter()
        .take(100)
        .collect();
    update_playlist(&spotify, tracks);
}
