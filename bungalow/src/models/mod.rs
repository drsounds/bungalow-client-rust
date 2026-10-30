//! Port of `bungalow/include/models`: the ORM model classes.

pub mod sp_artist;
pub mod sp_audio_file;
pub mod sp_db_connection;
pub mod sp_file_row;
pub mod sp_label;
pub mod sp_model;
pub mod sp_playable;
pub mod sp_playable_track;
pub mod sp_recording;
pub mod sp_recording_artist;
pub mod sp_release;
pub mod sp_release_artist;
pub mod sp_save;
pub mod sp_track;
pub mod sp_track_audio_file;

pub use sp_artist::SPArtist;
pub use sp_audio_file::SPAudioFile;
pub use sp_db_connection::SPDBConnection;
pub use sp_file_row::SPFileRow;
pub use sp_label::SPLabel;
pub use sp_model::SPModel;
pub use sp_playable::SPPlayable;
pub use sp_playable_track::SPPlayableTrack;
pub use sp_recording::SPRecording;
pub use sp_recording_artist::SPRecordingArtist;
pub use sp_release::SPRelease;
pub use sp_release_artist::SPReleaseArtist;
pub use sp_save::SPSave;
pub use sp_track::SPTrack;
pub use sp_track_audio_file::SPTrackAudioFile;
