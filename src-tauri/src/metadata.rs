//! Audio metadata reader/writer using `lofty`. Replaces both
//! `music-metadata` and `music-tag-native` from the original Node app.

use std::path::Path;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::Accessor;
use lofty::tag::{ItemKey, Tag, TagExt};

use crate::types::{CoverArt, TrackMetadata};

#[derive(Debug, Clone)]
pub struct ParsedMetadata {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub duration: f64,
    pub track_number: Option<i64>,
    pub disk_number: Option<i64>,
    pub year: Option<i64>,
    pub format: Option<String>,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub has_cover: bool,
}

fn derive_title_from_path(p: &Path) -> String {
    p.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn read_tag_string(tag: &Tag, key: ItemKey) -> Option<String> {
    tag.get_string(&key).map(|s| s.to_string())
}

pub fn parse_track_metadata(file_path: &Path) -> Option<ParsedMetadata> {
    let tagged = lofty::read_from_path(file_path).ok()?;

    let props = tagged.properties();
    let tag = tagged.primary_tag()?;

    let title = tag
        .title()
        .map(|s| s.to_string())
        .unwrap_or_else(|| derive_title_from_path(file_path));
    let artist = tag.artist().map(|s| s.to_string());
    let album = tag.album().map(|s| s.to_string());
    let album_artist = read_tag_string(tag, ItemKey::AlbumArtist).or_else(|| artist.clone());

    let track_number = tag.track().map(|t| t as i64);
    let disk_number = tag.disk().map(|d| d as i64);
    let year = tag.year().map(|y| y as i64);

    let duration = props.duration().as_secs_f64();
    let bitrate = props.audio_bitrate().map(|b| (b / 1000) as i64);
    let sample_rate = props.sample_rate().map(|s| s as i64);
    let format = format!("{:?}", tagged.file_type());

    let has_cover = !tag.pictures().is_empty();

    Some(ParsedMetadata {
        title,
        artist,
        album,
        album_artist,
        duration,
        track_number,
        disk_number,
        year,
        format: Some(format),
        bitrate,
        sample_rate,
        has_cover,
    })
}

pub fn read_track_metadata(track_path: &Path) -> Result<TrackMetadata, String> {
    let tagged = lofty::read_from_path(track_path)
        .map_err(|e| format!("failed to read audio file: {e}"))?;
    let tag = tagged
        .primary_tag()
        .ok_or("no tag found")?;

    let first_pic = tag.pictures().first();
    let cover_art = first_pic.map(|pic| {
        let mime = pic
            .mime_type()
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "image/jpeg".into());
        let data = BASE64.encode(pic.data());
        CoverArt { mime_type: mime, data }
    });

    Ok(TrackMetadata {
        title: tag.title().map(|s| s.to_string()).unwrap_or_default(),
        artist: tag.artist().map(|s| s.to_string()).unwrap_or_default(),
        album: tag.album().map(|s| s.to_string()).unwrap_or_default(),
        album_artist: read_tag_string(tag, ItemKey::AlbumArtist).unwrap_or_default(),
        year: tag.year().map(|y| y as i64),
        track_number: tag.track().map(|t| t as i64),
        cover_art,
    })
}

#[derive(Debug, Clone, Default)]
pub struct MetadataUpdate {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub year: Option<Option<i64>>,
    pub track_number: Option<Option<i64>>,
    pub cover_art: Option<Option<CoverArt>>,
}

pub fn update_track_metadata(track_path: &Path, update: &MetadataUpdate) -> Result<(), String> {
    let mut tagged = lofty::read_from_path(track_path)
        .map_err(|e| format!("failed to read audio file: {e}"))?;

    let tag = tagged
        .primary_tag_mut()
        .ok_or("no writable primary tag found")?;

    if let Some(v) = &update.title {
        tag.set_title(v.clone());
    }
    if let Some(v) = &update.artist {
        tag.set_artist(v.clone());
    }
    if let Some(v) = &update.album {
        tag.set_album(v.clone());
    }
    if let Some(v) = &update.album_artist {
        tag.insert_text(ItemKey::AlbumArtist, v.clone());
    }
    if let Some(v) = &update.year {
        if let Some(y) = v {
            tag.set_year(*y as u32);
        } else {
            tag.remove_key(&ItemKey::Year);
        }
    }
    if let Some(v) = &update.track_number {
        if let Some(n) = v {
            tag.set_track(*n as u32);
        } else {
            tag.remove_key(&ItemKey::TrackNumber);
        }
    }

    match &update.cover_art {
        Some(None) => {
            while tag.pictures().len() > 0 {
                tag.remove_picture(0);
            }
        }
        Some(Some(cover)) => {
            let bytes = BASE64
                .decode(&cover.data)
                .map_err(|e| format!("bad base64 cover: {e}"))?;
            let mime = MimeType::from_str(&cover.mime_type);
            let picture = Picture::new_unchecked(
                PictureType::CoverFront,
                Some(mime),
                Some(String::new()),
                bytes,
            );
            while tag.pictures().len() > 0 {
                tag.remove_picture(0);
            }
            tag.push_picture(picture);
        }
        None => {}
    }

    // Save the TaggedFile back to disk (writes all tags it holds).
    tagged
        .save_to_path(track_path, WriteOptions::new())
        .map_err(|e| format!("failed to write tags: {e}"))?;

    Ok(())
}

pub fn extract_cover_from_path(path: &Path) -> Result<(PictureType, Vec<u8>, String), String> {
    let tagged = lofty::read_from_path(path)
        .map_err(|e| format!("failed to read audio file: {e}"))?;
    let tag = tagged.primary_tag().ok_or("no tag")?;
    let pic = tag.pictures().first().ok_or("no embedded cover")?;
    let mime = pic
        .mime_type()
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "image/jpeg".into());
    Ok((pic.pic_type(), pic.data().to_vec(), mime))
}
