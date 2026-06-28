//! Protobuf definitions for the Spotify Canvas API.
//! Faithful representation of `server/src/services/proto/_canvas.proto`.

#[derive(Clone, PartialEq, prost::Message)]
pub struct CanvasRequest {
    #[prost(message, repeated, tag = "1")]
    pub tracks: Vec<canvas_request::Track>,
}

pub mod canvas_request {
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct Track {
        #[prost(string, tag = "1")]
        pub track_uri: String,
    }
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct CanvasResponse {
    #[prost(message, repeated, tag = "1")]
    pub canvases: Vec<canvas_response::Canvas>,
}

pub mod canvas_response {
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct Canvas {
        #[prost(string, tag = "1")]
        pub id: String,
        #[prost(string, tag = "2")]
        pub canvas_url: String,
        #[prost(string, tag = "5")]
        pub track_uri: String,
        #[prost(message, optional, tag = "6")]
        pub artist: Option<canvas::Artist>,
        #[prost(string, tag = "9")]
        pub other_id: String,
        #[prost(string, tag = "11")]
        pub canvas_uri: String,
    }

    pub mod canvas {
        #[derive(Clone, PartialEq, prost::Message)]
        pub struct Artist {
            #[prost(string, tag = "1")]
            pub artist_uri: String,
            #[prost(string, tag = "2")]
            pub artist_name: String,
            #[prost(string, tag = "3")]
            pub artist_img_url: String,
        }
    }
}
