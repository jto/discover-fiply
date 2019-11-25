pub mod fip_client;
use std::time::SystemTime;

fn main() {
    env_logger::init();
    println!("Hello, world!");
    let (songs, page) = fip_client::fetch_songs(SystemTime::now()).unwrap();
    for s in &songs {
        println!("{:?}", *s);
    }
}
