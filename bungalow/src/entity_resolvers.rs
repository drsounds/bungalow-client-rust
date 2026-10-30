//! Port of `EntityResolvers`: registers resolvers that turn bungalow: entity
//! URIs into JSON from the local library database:
//!
//!   bungalow:artist:<isni>  {uri, type, isni, name}
//!   bungalow:track:<id>     {uri, type, id, name, version, isrc, release, artists}

use std::rc::Rc;

use serde_json::{json, Value};
use space::sp_uri_store::SPUriStore;

use crate::database::{ArtistRelationship, Database};

const ARTIST_PREFIX: &str = "bungalow:artist:";
const TRACK_PREFIX: &str = "bungalow:track:";

fn id_from_uri<'a>(uri: &'a str, prefix: &str) -> &'a str {
    let id = uri.get(prefix.len()..).unwrap_or_default();
    id.split([':', '#']).next().unwrap_or_default()
}

fn artists_to_json(artists: &[ArtistRelationship]) -> Value {
    Value::Array(
        artists
            .iter()
            .map(|artist| {
                json!({
                    "uri": format!("{ARTIST_PREFIX}{}", artist.artist_isni),
                    "isni": artist.artist_isni,
                    "name": artist.artist_name,
                    "primary": artist.is_primary,
                })
            })
            .collect(),
    )
}

/// SQLite lookups are fast enough to resolve synchronously on the UI
/// thread; slow (network) resolvers should resolve from a worker.
pub fn register_entity_resolvers(store: &SPUriStore, database: Rc<Database>) {
    let db = database.clone();
    store.register_resolver(
        ARTIST_PREFIX,
        Rc::new(move |uri, resolve, reject| {
            let artist = db.get_artist(id_from_uri(uri, ARTIST_PREFIX));
            if artist.isni.is_empty() {
                reject("Artist not found".into());
                return;
            }
            resolve(json!({
                "uri": uri,
                "type": "artist",
                "isni": artist.isni,
                "name": artist.name,
            }));
        }),
    );

    let db = database;
    store.register_resolver(
        TRACK_PREFIX,
        Rc::new(move |uri, resolve, reject| {
            let Ok(track_id) = id_from_uri(uri, TRACK_PREFIX).parse::<i32>() else {
                reject("Invalid track id".into());
                return;
            };
            let track = db.get_track_details(track_id);
            if track.recording.isrc.is_empty() {
                reject("Track not found".into());
                return;
            }
            let release = if track.release.upc.is_empty() {
                Value::Null
            } else {
                json!({
                    "uri": format!("bungalow:release:{}", track.release.upc),
                    "upc": track.release.upc,
                    "name": track.release.name,
                    "releaseDate": track.release.release_date,
                    "artists": artists_to_json(&track.release_artists),
                })
            };
            resolve(json!({
                "uri": uri,
                "type": "track",
                "id": track.id,
                "name": track.recording.name,
                "version": track.recording.version,
                "isrc": track.recording.isrc,
                "release": release,
                "artists": artists_to_json(&track.recording_artists),
            }));
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ids() {
        assert_eq!(id_from_uri("bungalow:track:12:extra", TRACK_PREFIX), "12");
        assert_eq!(id_from_uri("bungalow:artist:0001#top", ARTIST_PREFIX), "0001");
    }
}
